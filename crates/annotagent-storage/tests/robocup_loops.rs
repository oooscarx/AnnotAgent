use std::sync::Arc;

use annotagent_core::{
    Budget, PricingConfig, ProjectId, ProjectSchema, RunEventKind, RunId, RunStatus, TaskRunStatus,
};
use annotagent_image_tools::{generate_synthetic_robocup, load_image};
use annotagent_provider::{
    MockResponseSpec, MockScript, MockStep, MockToolCall, MockUsage, MockVisionProvider,
};
use annotagent_runtime::{AgentLoopConfig, AgentRuntime, ImageRunRequest};
use annotagent_skill_robocup::RoboCupSkill;
use annotagent_storage::SqliteStore;
use serde_json::json;

fn project() -> ProjectSchema {
    let project = ProjectSchema::from_yaml(include_str!("../../../examples/robocup/project.yaml"))
        .expect("ball-only example project");
    assert_eq!(project.tasks.len(), 1);
    assert_eq!(project.tasks[0].id.as_str(), "objects");
    assert_eq!(
        project.tasks[0]
            .labels
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["ball"]
    );
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
    AgentRuntime::new(
        Arc::new(RoboCupSkill::new().expect("RoboCup Ball skill")),
        provider,
        store,
        PricingConfig::default(),
        Budget {
            max_requests: Some(10),
            ..Budget::default()
        },
        AgentLoopConfig {
            max_model_turns_per_task: 4,
            max_retries: 2,
            ..AgentLoopConfig::default()
        },
    )
}

async fn run(
    provider: Arc<MockVisionProvider>,
    store: Arc<SqliteStore>,
) -> annotagent_runtime::ImageRunResult {
    let (temporary, image) = fixture();
    runtime(provider, store)
        .run_image(ImageRunRequest {
            run_id: RunId::new(),
            project_id: ProjectId::new(),
            project_root: temporary.path().to_path_buf(),
            project: Arc::new(project()),
            image_id: annotagent_core::ImageId::new(),
            image,
            model_image: None,
        })
        .await
        .expect("ball run")
}

fn step(response: MockResponseSpec) -> MockStep {
    MockStep {
        expect_task: Some("objects".to_owned()),
        expect_message_contains: None,
        response,
        usage: MockUsage {
            input_tokens: 100,
            output_tokens: 30,
        },
    }
}

fn ball_submission(rect: [f64; 4]) -> MockResponseSpec {
    MockResponseSpec::ToolCall {
        name: "submit_annotation_candidates".to_owned(),
        arguments: json!({"annotations": [{
            "label": "ball",
            "value": {"kind": "bounding_box", "rect": rect},
            "attributes": {},
            "confidence": 0.98
        }]}),
    }
}

#[tokio::test]
async fn ball_evidence_reaches_the_model_before_submission() {
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![
            step(MockResponseSpec::ToolCall {
                name: "evaluate_ball_hard_negative".to_owned(),
                arguments: json!({"bbox": [0.547, 0.75, 0.038, 0.06]}),
            }),
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: Some("white_ratio".to_owned()),
                response: ball_submission([0.547, 0.75, 0.038, 0.06]),
                usage: MockUsage {
                    input_tokens: 140,
                    output_tokens: 35,
                },
            },
        ],
    }));
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite"));
    let run_id = RunId::new();
    let (temporary, image) = fixture();
    let result = runtime(provider, store.clone())
        .run_image(ImageRunRequest {
            run_id,
            project_id: ProjectId::new(),
            project_root: temporary.path().to_path_buf(),
            project: Arc::new(project()),
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
    let assistant_index = history
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
    let tool_result = &history.model_messages[assistant_index + 1].message;
    assert_eq!(tool_result.role, annotagent_core::ModelRole::Tool);
    assert!(tool_result.content.contains("white_ratio"));
}

#[tokio::test]
async fn absent_ball_is_a_succeeded_empty_task() {
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![step(MockResponseSpec::ToolCall {
            name: "finish_task".to_owned(),
            arguments: json!({}),
        })],
    }));
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite"));
    let run_id = RunId::new();
    let (temporary, image) = fixture();
    let result = runtime(provider, store.clone())
        .run_image(ImageRunRequest {
            run_id,
            project_id: ProjectId::new(),
            project_root: temporary.path().to_path_buf(),
            project: Arc::new(project()),
            image_id: annotagent_core::ImageId::new(),
            image,
            model_image: None,
        })
        .await
        .expect("valid empty task");

    assert_eq!(result.status, RunStatus::Completed, "{result:#?}");
    assert!(result.committed.is_empty());
    assert_eq!(
        store.list_task_runs(run_id).expect("task status")[0].status,
        TaskRunStatus::SucceededEmpty
    );
}

#[tokio::test]
async fn identical_ball_evidence_call_reuses_cached_result() {
    let evidence_call = MockToolCall {
        name: "evaluate_ball_hard_negative".to_owned(),
        arguments: json!({"bbox": [0.547, 0.75, 0.038, 0.06]}),
    };
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![
            step(MockResponseSpec::ToolCalls {
                calls: vec![evidence_call.clone(), evidence_call],
                content: None,
            }),
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: Some(
                    "identical deterministic tool call reused".to_owned(),
                ),
                response: ball_submission([0.547, 0.75, 0.038, 0.06]),
                usage: MockUsage {
                    input_tokens: 140,
                    output_tokens: 35,
                },
            },
        ],
    }));
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite"));
    let result = run(provider, store.clone()).await;

    assert_eq!(result.status, RunStatus::Completed, "{result:#?}");
    let history = store.history(result.run_id).expect("history");
    assert!(history.tool_calls.iter().any(|call| {
        call.result
            .as_ref()
            .is_some_and(|result| result.ui_summary.starts_with("cache hit"))
    }));
}

#[tokio::test]
async fn repeated_auxiliary_calls_reserve_a_bounded_convergence_turn() {
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![
            step(MockResponseSpec::ToolCall {
                name: "evaluate_ball_hard_negative".to_owned(),
                arguments: json!({"bbox": [0.20, 0.30, 0.04, 0.04]}),
            }),
            step(MockResponseSpec::ToolCall {
                name: "evaluate_ball_hard_negative".to_owned(),
                arguments: json!({"bbox": [0.547, 0.75, 0.038, 0.06]}),
            }),
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: Some("convergence_required".to_owned()),
                response: ball_submission([0.547, 0.75, 0.038, 0.06]),
                usage: MockUsage {
                    input_tokens: 140,
                    output_tokens: 35,
                },
            },
        ],
    }));
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite"));
    let result = run(provider, store.clone()).await;

    assert_eq!(result.status, RunStatus::Completed, "{result:#?}");
    assert_eq!(result.committed.len(), 1);
    assert_eq!(result.usage.requests, 3);
    assert!(
        store
            .list_events(result.run_id)
            .expect("events")
            .iter()
            .any(|event| {
                event.kind == RunEventKind::RetryScheduled
                    && matches!(
                        &event.payload,
                        annotagent_core::RunEventPayload::Message { summary }
                            if summary.contains("bounded convergence turn")
                    )
            })
    );
}

#[tokio::test]
async fn unusual_ball_geometry_is_retried_and_corrected() {
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![
            step(ball_submission([0.547, 0.75, 0.06, 0.01])),
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: Some("unlikely_ball_geometry".to_owned()),
                response: ball_submission([0.547, 0.75, 0.038, 0.06]),
                usage: MockUsage {
                    input_tokens: 140,
                    output_tokens: 35,
                },
            },
        ],
    }));
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite"));
    let result = run(provider, store.clone()).await;

    assert_eq!(result.status, RunStatus::Completed, "{result:#?}");
    assert_eq!(result.committed.len(), 1);
    assert_eq!(result.usage.requests, 2);
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.code == "unlikely_ball_geometry")
    );
    assert!(
        store
            .list_events(result.run_id)
            .expect("events")
            .iter()
            .any(|event| event.kind == RunEventKind::RetryScheduled)
    );
}

#[tokio::test]
async fn multiple_ball_tool_calls_are_persisted_and_answered_in_order() {
    let calls = [
        [0.20, 0.30, 0.04, 0.04],
        [0.40, 0.50, 0.04, 0.04],
        [0.547, 0.75, 0.038, 0.06],
    ]
    .map(|bbox| MockToolCall {
        name: "evaluate_ball_hard_negative".to_owned(),
        arguments: json!({"bbox": bbox}),
    })
    .to_vec();
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![
            step(MockResponseSpec::ToolCalls {
                calls,
                content: None,
            }),
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: Some("white_ratio".to_owned()),
                response: ball_submission([0.547, 0.75, 0.038, 0.06]),
                usage: MockUsage {
                    input_tokens: 140,
                    output_tokens: 35,
                },
            },
        ],
    }));
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite"));
    let run_id = RunId::new();
    let (temporary, image) = fixture();
    let result = runtime(provider, store.clone())
        .run_image(ImageRunRequest {
            run_id,
            project_id: ProjectId::new(),
            project_root: temporary.path().to_path_buf(),
            project: Arc::new(project()),
            image_id: annotagent_core::ImageId::new(),
            image,
            model_image: None,
        })
        .await
        .expect("multi-tool ball loop");

    assert_eq!(result.status, RunStatus::Completed, "{result:#?}");
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
