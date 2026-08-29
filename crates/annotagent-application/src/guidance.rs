use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::ProjectReadiness;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStage {
    NeedsData,
    NeedsLabels,
    NeedsAutomation,
    NeedsModelBinding,
    ReadyForSampleTest,
    SampleTestNeedsAttention,
    ReadyToActivate,
    ReadyToRun,
    Running,
    NeedsReview,
    ReadyToExport,
    ConfigurationIssue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuidedActionKind {
    AddImages,
    DefineLabels,
    ChooseAutomation,
    ConnectModel,
    FixAutomation,
    TestSamples,
    ReviewTestResults,
    ActivateAutomation,
    RunDataset,
    OpenActiveRun,
    ReviewResults,
    ExportDataset,
    ViewAutomation,
    ViewRuns,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuidedAction {
    pub kind: GuidedActionKind,
    pub label: String,
    pub destination: Option<String>,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuidanceBlocker {
    pub code: String,
    pub title: String,
    pub explanation: String,
    pub repair_action: Option<GuidedAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectJourneyState {
    Complete,
    Current,
    Upcoming,
    NeedsAttention,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectJourneyStep {
    pub id: String,
    pub label: String,
    pub state: ProjectJourneyState,
    pub detail: String,
    pub destination: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectGuidance {
    pub project_id: String,
    pub stage: ProjectStage,
    pub completed_steps: u32,
    pub total_steps: u32,
    pub headline: String,
    pub explanation: String,
    pub primary_action: GuidedAction,
    pub secondary_actions: Vec<GuidedAction>,
    pub blockers: Vec<GuidanceBlocker>,
    pub journey: Vec<ProjectJourneyStep>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectReadinessSummary {
    pub project_id: String,
    pub readiness: ProjectReadiness,
    pub stage: ProjectStage,
    pub blockers: Vec<GuidanceBlocker>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleTestState {
    NotRun,
    Passed,
    NeedsAttention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGuidanceInput {
    pub project_id: String,
    pub image_count: usize,
    pub has_labels: bool,
    pub has_automation: bool,
    pub has_model_binding: bool,
    pub automation_valid: bool,
    pub sample_test: SampleTestState,
    pub automation_activated: bool,
    pub active_run_id: Option<String>,
    pub active_batch_id: Option<String>,
    pub review_count: usize,
    pub has_completed_run: bool,
    pub updated_at: DateTime<Utc>,
}

impl ProjectGuidance {
    #[must_use]
    pub fn readiness(&self) -> ProjectReadiness {
        match self.stage {
            ProjectStage::ConfigurationIssue => ProjectReadiness::ConfigurationIssue,
            ProjectStage::ReadyToRun
            | ProjectStage::Running
            | ProjectStage::NeedsReview
            | ProjectStage::ReadyToExport => ProjectReadiness::Ready,
            ProjectStage::NeedsData
            | ProjectStage::NeedsLabels
            | ProjectStage::NeedsAutomation
            | ProjectStage::NeedsModelBinding
            | ProjectStage::ReadyForSampleTest
            | ProjectStage::SampleTestNeedsAttention
            | ProjectStage::ReadyToActivate => ProjectReadiness::Incomplete,
        }
    }

    #[must_use]
    pub fn readiness_summary(&self) -> ProjectReadinessSummary {
        ProjectReadinessSummary {
            project_id: self.project_id.clone(),
            readiness: self.readiness(),
            stage: self.stage,
            blockers: self.blockers.clone(),
            updated_at: self.updated_at,
        }
    }
}

#[must_use]
pub fn derive_project_guidance(input: ProjectGuidanceInput) -> ProjectGuidance {
    const TOTAL_STEPS: u32 = 8;
    let project_path = format!("/projects/{}", input.project_id);
    let build_path = |step: &str| format!("{project_path}/build/{step}");
    let action = |kind, label: &str, destination: String| GuidedAction {
        kind,
        label: label.to_owned(),
        destination: Some(destination),
        enabled: true,
        disabled_reason: None,
    };
    let blocker = |code: &str, title: &str, explanation: &str, repair_action| GuidanceBlocker {
        code: code.to_owned(),
        title: title.to_owned(),
        explanation: explanation.to_owned(),
        repair_action: Some(repair_action),
    };

    let data_complete = input.image_count > 0;
    let labels_complete = input.has_labels;
    let automation_complete = input.has_automation;
    let binding_complete = input.has_automation && input.has_model_binding;
    let automation_ready = automation_complete && input.automation_valid && binding_complete;
    let sample_complete = input.sample_test == SampleTestState::Passed;
    let activation_complete = input.automation_activated;
    let run_complete = input.has_completed_run;
    let review_complete = input.has_completed_run && input.review_count == 0;
    let completed_steps = [
        data_complete,
        labels_complete,
        automation_ready,
        sample_complete,
        activation_complete,
        run_complete,
        review_complete,
    ]
    .into_iter()
    .filter(|complete| *complete)
    .count() as u32;

    let (stage, headline, explanation, primary_action, blockers) = if let Some(run_id) =
        input.active_run_id.as_deref()
    {
        (
            ProjectStage::Running,
            "Your dataset run is in progress.",
            "Open the active Run to follow results, cost, errors, and review work from server-owned state.",
            action(
                GuidedActionKind::OpenActiveRun,
                "Open active run",
                format!("/runs/{run_id}"),
            ),
            Vec::new(),
        )
    } else if input.active_batch_id.is_some() {
        (
            ProjectStage::Running,
            "Your dataset run is in progress.",
            "Open the Project to follow the durable Dataset Batch and its child Runs.",
            action(
                GuidedActionKind::OpenActiveRun,
                "Open active run",
                project_path.clone(),
            ),
            Vec::new(),
        )
    } else if !data_complete {
        let repair = action(
            GuidedActionKind::AddImages,
            "Add images",
            build_path("data"),
        );
        (
            ProjectStage::NeedsData,
            "Add images to start this Project.",
            "AnnotAgent needs at least one supported image before it can test or run an automation.",
            repair.clone(),
            vec![blocker(
                "no_images",
                "No images yet",
                "Import a workspace-local image or folder.",
                repair,
            )],
        )
    } else if !labels_complete {
        let repair = action(
            GuidedActionKind::DefineLabels,
            "Define labels",
            build_path("labels"),
        );
        (
            ProjectStage::NeedsLabels,
            "Tell AnnotAgent what to annotate.",
            "Create at least one user-facing Label and annotation type for this dataset.",
            repair.clone(),
            vec![blocker(
                "no_labels",
                "No Labels defined",
                "Add a Label group with at least one Label.",
                repair,
            )],
        )
    } else if !automation_complete {
        let repair = action(
            GuidedActionKind::ChooseAutomation,
            "Choose automation",
            build_path("pipeline"),
        );
        (
            ProjectStage::NeedsAutomation,
            "Choose how AnnotAgent should produce these Labels.",
            "Start from a recommended Automation Recipe or ask the bounded Advisor for a Draft.",
            repair.clone(),
            vec![blocker(
                "no_automation",
                "No Automation selected",
                "Create an editable Draft from a registered recipe.",
                repair,
            )],
        )
    } else if !input.automation_valid {
        let repair = action(
            GuidedActionKind::FixAutomation,
            "Fix automation",
            build_path("pipeline"),
        );
        (
            ProjectStage::ConfigurationIssue,
            "This Automation needs attention.",
            "Resolve its blocking type, connection, or policy issues before testing it on images.",
            repair.clone(),
            vec![blocker(
                "invalid_automation",
                "Automation validation failed",
                "Open the Draft and repair every blocking validation issue.",
                repair,
            )],
        )
    } else if !binding_complete {
        let repair = action(
            GuidedActionKind::ConnectModel,
            "Connect model",
            format!("/settings/models?return_to={project_path}"),
        );
        (
            ProjectStage::NeedsModelBinding,
            "Connect the model used by this Automation.",
            "The graph is valid, but at least one model node has no usable registered binding.",
            repair.clone(),
            vec![blocker(
                "missing_model_binding",
                "Model connection required",
                "Choose a registered model and complete its provider connection.",
                repair,
            )],
        )
    } else if input.sample_test == SampleTestState::NeedsAttention {
        let repair = action(
            GuidedActionKind::ReviewTestResults,
            "Review test results",
            build_path("test"),
        );
        (
            ProjectStage::SampleTestNeedsAttention,
            "The sample test found results that need attention.",
            "Inspect failed or uncertain samples, then adjust the same Automation Draft and test again.",
            repair.clone(),
            vec![blocker(
                "sample_test_needs_attention",
                "Sample test needs attention",
                "Open the result gallery and diagnostics before activation.",
                repair,
            )],
        )
    } else if input.sample_test == SampleTestState::NotRun && !activation_complete {
        (
            ProjectStage::ReadyForSampleTest,
            "Test the Automation on a few images.",
            "A sandbox sample test shows annotation outcomes without writing formal Annotations.",
            action(
                GuidedActionKind::TestSamples,
                "Test on samples",
                build_path("test"),
            ),
            Vec::new(),
        )
    } else if !activation_complete {
        (
            ProjectStage::ReadyToActivate,
            "The sample test is ready to activate.",
            "Publish the tested Draft as an immutable Automation Version for future Runs.",
            action(
                GuidedActionKind::ActivateAutomation,
                "Activate automation",
                build_path("test"),
            ),
            Vec::new(),
        )
    } else if input.review_count > 0 {
        (
            ProjectStage::NeedsReview,
            "Some annotations need your decision.",
            "Review uncertain or conflicting results before exporting the dataset.",
            action(
                GuidedActionKind::ReviewResults,
                "Review results",
                format!("/review?project_id={}", input.project_id),
            ),
            Vec::new(),
        )
    } else if input.has_completed_run {
        (
            ProjectStage::ReadyToExport,
            "Your reviewed annotations are ready to export.",
            "Choose a format compatible with the Project Schema and keep the generated report with the dataset.",
            action(
                GuidedActionKind::ExportDataset,
                "Export dataset",
                format!("{project_path}/export"),
            ),
            Vec::new(),
        )
    } else {
        (
            ProjectStage::ReadyToRun,
            "Run the active Automation on your dataset.",
            "The Project has data, Labels, a tested model connection, and an immutable Automation Version.",
            action(
                GuidedActionKind::RunDataset,
                "Run dataset",
                project_path.clone(),
            ),
            Vec::new(),
        )
    };

    let mut secondary_actions = Vec::new();
    if automation_complete
        && !matches!(
            primary_action.kind,
            GuidedActionKind::ChooseAutomation | GuidedActionKind::FixAutomation
        )
    {
        secondary_actions.push(action(
            GuidedActionKind::ViewAutomation,
            "View automation",
            build_path("pipeline"),
        ));
    }
    if (input.has_completed_run || input.active_run_id.is_some() || input.active_batch_id.is_some())
        && secondary_actions.len() < 2
    {
        secondary_actions.push(action(
            GuidedActionKind::ViewRuns,
            "View runs",
            format!("/runs?project_id={}", input.project_id),
        ));
    }

    let journey_step = |id: &str,
                        label: &str,
                        state: ProjectJourneyState,
                        detail: String,
                        destination: Option<String>| ProjectJourneyStep {
        id: id.to_owned(),
        label: label.to_owned(),
        state,
        detail,
        destination,
    };
    let journey = vec![
        journey_step(
            "data",
            "Data",
            if data_complete {
                ProjectJourneyState::Complete
            } else if stage == ProjectStage::NeedsData {
                ProjectJourneyState::Current
            } else {
                ProjectJourneyState::Upcoming
            },
            if data_complete {
                format!("Complete · {} images", input.image_count)
            } else {
                "Add at least one supported image".to_owned()
            },
            Some(build_path("data")),
        ),
        journey_step(
            "labels",
            "Labels",
            if labels_complete {
                ProjectJourneyState::Complete
            } else if stage == ProjectStage::NeedsLabels {
                ProjectJourneyState::Current
            } else {
                ProjectJourneyState::Upcoming
            },
            if labels_complete {
                "Complete · Label Schema defined".to_owned()
            } else {
                "Define what the Project should annotate".to_owned()
            },
            Some(build_path("labels")),
        ),
        journey_step(
            "automation",
            "Automation",
            if automation_ready {
                ProjectJourneyState::Complete
            } else if matches!(
                stage,
                ProjectStage::NeedsModelBinding | ProjectStage::ConfigurationIssue
            ) {
                ProjectJourneyState::NeedsAttention
            } else if stage == ProjectStage::NeedsAutomation {
                ProjectJourneyState::Current
            } else {
                ProjectJourneyState::Upcoming
            },
            if automation_ready {
                "Draft ready · model connection available".to_owned()
            } else if automation_complete && !input.automation_valid {
                "Resolve blocking validation issues".to_owned()
            } else if automation_complete && !binding_complete {
                "Connect the model used by this Draft".to_owned()
            } else {
                "Choose a recipe or Advisor proposal".to_owned()
            },
            Some(build_path("pipeline")),
        ),
        journey_step(
            "sample_test",
            "Sample test",
            if sample_complete {
                ProjectJourneyState::Complete
            } else if stage == ProjectStage::SampleTestNeedsAttention {
                ProjectJourneyState::NeedsAttention
            } else if stage == ProjectStage::ReadyForSampleTest {
                ProjectJourneyState::Current
            } else {
                ProjectJourneyState::Upcoming
            },
            match input.sample_test {
                SampleTestState::Passed => "Complete · sandbox checks passed".to_owned(),
                SampleTestState::NeedsAttention => "Results need attention".to_owned(),
                SampleTestState::NotRun => "Not run".to_owned(),
            },
            Some(build_path("test")),
        ),
        journey_step(
            "activation",
            "Activation",
            if activation_complete {
                ProjectJourneyState::Complete
            } else if stage == ProjectStage::ReadyToActivate {
                ProjectJourneyState::Current
            } else {
                ProjectJourneyState::Upcoming
            },
            if activation_complete {
                "Complete · immutable Version active".to_owned()
            } else {
                "Not activated".to_owned()
            },
            Some(build_path("test")),
        ),
        journey_step(
            "full_run",
            "Full run",
            if run_complete {
                ProjectJourneyState::Complete
            } else if matches!(stage, ProjectStage::ReadyToRun | ProjectStage::Running) {
                ProjectJourneyState::Current
            } else {
                ProjectJourneyState::Upcoming
            },
            if run_complete {
                "Complete · results persisted".to_owned()
            } else if input.active_run_id.is_some() || input.active_batch_id.is_some() {
                "In progress".to_owned()
            } else {
                "Not started".to_owned()
            },
            Some(format!("/runs?project_id={}", input.project_id)),
        ),
        journey_step(
            "review",
            "Review",
            if review_complete {
                ProjectJourneyState::Complete
            } else if stage == ProjectStage::NeedsReview {
                ProjectJourneyState::Current
            } else {
                ProjectJourneyState::Upcoming
            },
            if input.review_count > 0 {
                format!("{} items need a decision", input.review_count)
            } else if run_complete {
                "Complete · no unresolved items".to_owned()
            } else {
                "No items yet".to_owned()
            },
            Some(format!("/review?project_id={}", input.project_id)),
        ),
        journey_step(
            "export",
            "Export",
            if stage == ProjectStage::ReadyToExport {
                ProjectJourneyState::Ready
            } else {
                ProjectJourneyState::Upcoming
            },
            if run_complete && input.review_count == 0 {
                "Ready for a compatible format".to_owned()
            } else {
                "Not ready".to_owned()
            },
            Some(format!("{project_path}/export")),
        ),
    ];

    ProjectGuidance {
        project_id: input.project_id,
        stage,
        completed_steps,
        total_steps: TOTAL_STEPS,
        headline: headline.to_owned(),
        explanation: explanation.to_owned(),
        primary_action,
        secondary_actions,
        blockers,
        journey,
        updated_at: input.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn ready_input() -> ProjectGuidanceInput {
        ProjectGuidanceInput {
            project_id: "vision-components".to_owned(),
            image_count: 5,
            has_labels: true,
            has_automation: true,
            has_model_binding: true,
            automation_valid: true,
            sample_test: SampleTestState::Passed,
            automation_activated: true,
            active_run_id: None,
            active_batch_id: None,
            review_count: 0,
            has_completed_run: false,
            updated_at: Utc.with_ymd_and_hms(2026, 8, 30, 0, 0, 0).unwrap(),
        }
    }

    fn assert_stage(input: ProjectGuidanceInput, stage: ProjectStage, action: GuidedActionKind) {
        let guidance = derive_project_guidance(input);
        assert_eq!(guidance.stage, stage);
        assert_eq!(guidance.primary_action.kind, action);
        assert!(guidance.primary_action.enabled);
        assert_eq!(guidance.total_steps, 8);
        assert_eq!(guidance.journey.len(), 8);
        assert!(guidance.secondary_actions.len() <= 2);
        assert_eq!(
            guidance
                .journey
                .iter()
                .filter(|step| matches!(
                    step.state,
                    ProjectJourneyState::Current
                        | ProjectJourneyState::NeedsAttention
                        | ProjectJourneyState::Ready
                ))
                .count(),
            1
        );
    }

    #[test]
    fn every_primary_journey_state_has_one_deterministic_action() {
        let mut needs_data = ready_input();
        needs_data.image_count = 0;
        assert_stage(
            needs_data,
            ProjectStage::NeedsData,
            GuidedActionKind::AddImages,
        );

        let mut needs_labels = ready_input();
        needs_labels.has_labels = false;
        assert_stage(
            needs_labels,
            ProjectStage::NeedsLabels,
            GuidedActionKind::DefineLabels,
        );

        let mut needs_automation = ready_input();
        needs_automation.has_automation = false;
        assert_stage(
            needs_automation,
            ProjectStage::NeedsAutomation,
            GuidedActionKind::ChooseAutomation,
        );

        let mut needs_model = ready_input();
        needs_model.has_model_binding = false;
        assert_stage(
            needs_model,
            ProjectStage::NeedsModelBinding,
            GuidedActionKind::ConnectModel,
        );

        let mut needs_test = ready_input();
        needs_test.sample_test = SampleTestState::NotRun;
        needs_test.automation_activated = false;
        assert_stage(
            needs_test,
            ProjectStage::ReadyForSampleTest,
            GuidedActionKind::TestSamples,
        );

        let mut bad_test = ready_input();
        bad_test.sample_test = SampleTestState::NeedsAttention;
        bad_test.automation_activated = false;
        assert_stage(
            bad_test,
            ProjectStage::SampleTestNeedsAttention,
            GuidedActionKind::ReviewTestResults,
        );

        let mut activate = ready_input();
        activate.automation_activated = false;
        assert_stage(
            activate,
            ProjectStage::ReadyToActivate,
            GuidedActionKind::ActivateAutomation,
        );

        assert_stage(
            ready_input(),
            ProjectStage::ReadyToRun,
            GuidedActionKind::RunDataset,
        );

        let mut running = ready_input();
        running.active_run_id = Some("run-42".to_owned());
        assert_stage(
            running,
            ProjectStage::Running,
            GuidedActionKind::OpenActiveRun,
        );

        let mut review = ready_input();
        review.review_count = 3;
        review.has_completed_run = true;
        assert_stage(
            review,
            ProjectStage::NeedsReview,
            GuidedActionKind::ReviewResults,
        );

        let mut export = ready_input();
        export.has_completed_run = true;
        assert_stage(
            export,
            ProjectStage::ReadyToExport,
            GuidedActionKind::ExportDataset,
        );
    }

    #[test]
    fn invalid_automation_precedes_binding_and_testing() {
        let mut input = ready_input();
        input.automation_valid = false;
        input.has_model_binding = false;
        input.sample_test = SampleTestState::NeedsAttention;
        let guidance = derive_project_guidance(input);
        assert_eq!(guidance.stage, ProjectStage::ConfigurationIssue);
        assert_eq!(
            guidance.primary_action.kind,
            GuidedActionKind::FixAutomation
        );
        assert_eq!(guidance.readiness(), ProjectReadiness::ConfigurationIssue);
        assert_eq!(guidance.blockers.len(), 1);
        assert!(guidance.blockers[0].repair_action.is_some());
    }

    #[test]
    fn active_run_is_server_priority_even_when_reviews_exist() {
        let mut input = ready_input();
        input.active_run_id = Some("active-run".to_owned());
        input.review_count = 8;
        input.has_completed_run = true;
        let first = derive_project_guidance(input.clone());
        let second = derive_project_guidance(input);
        assert_eq!(first, second);
        assert_eq!(first.stage, ProjectStage::Running);
        assert_eq!(
            first.primary_action.destination.as_deref(),
            Some("/runs/active-run")
        );
    }
}
