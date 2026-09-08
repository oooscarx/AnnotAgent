use annotagent_application::{LocalApplication, load_settings};
use annotagent_core::{PipelineBuilderConstraints, WorkflowConstraints};
use tokio_util::sync::CancellationToken;

/// Integration dependencies compile without cfg(test), as used by the normal CLI/TUI.
#[tokio::test]
async fn offline_advisor_does_not_break_a_valid_plan_to_demonstrate_repair() {
    let workspace = tempfile::tempdir().expect("isolated TEST workspace");
    let app = LocalApplication::new(workspace.path()).expect("application");
    app.create_project("test-advisor", "version: 1\nproject:\n  name: TEST offline advisor\ndataset:\n  root: images\nruntime: {}\ntasks:\n  - id: scene\n    kind: classification\n    labels: [day, night]\n    required: true\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n").expect("project");
    annotagent_image_tools::generate_synthetic_inspection(
        &workspace.path().join("test-advisor/images/test.png"),
    )
    .expect("TEST image");
    let report = app
        .run_workflow_advisor_agent(
            "test-advisor",
            &load_settings(None).expect("settings"),
            &WorkflowConstraints::default(),
            Some(("scene", "day")),
            PipelineBuilderConstraints::default(),
            CancellationToken::new(),
        )
        .await
        .expect("offline advisor");
    assert!(report.approval_required);
    assert!(report.validation.as_ref().is_some_and(|value| value.valid));
    assert!(report.session.steps.iter().all(|step| !matches!(
        step.tool_name.as_str(),
        "disconnect_pipeline_nodes" | "connect_pipeline_nodes"
    )));
    let validations = report
        .session
        .steps
        .iter()
        .filter(|step| step.tool_name == "validate_pipeline")
        .collect::<Vec<_>>();
    assert!(!validations.is_empty());
    assert!(
        validations
            .iter()
            .all(|step| step.result["model_payload"]["valid"] == true)
    );
}
