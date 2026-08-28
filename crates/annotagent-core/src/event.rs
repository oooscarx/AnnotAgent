//! Versioned events and run-state transitions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{AnnotationId, ArtifactId, EventId, ImageId, RunId, TaskId, ToolCallId, UsageTotals};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    Paused,
    AwaitingReview,
    Completed,
    CompletedWithReview,
    Partial,
    Cancelled,
    BudgetExceeded,
    Failed,
    Interrupted,
}

impl RunStatus {
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        use RunStatus::{
            AwaitingReview, BudgetExceeded, Cancelled, Completed, CompletedWithReview, Failed,
            Interrupted, Partial, Paused, Pending, Running,
        };
        matches!(
            (self, next),
            (Pending | Paused, Running | Cancelled)
                | (
                    AwaitingReview,
                    Running | Completed | CompletedWithReview | Cancelled | Interrupted
                )
                | (CompletedWithReview, Completed)
                | (
                    Running,
                    Paused
                        | AwaitingReview
                        | Completed
                        | CompletedWithReview
                        | Partial
                        | Cancelled
                        | BudgetExceeded
                        | Failed
                        | Interrupted
                )
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskRunStatus {
    Pending,
    Running,
    Succeeded,
    SucceededEmpty,
    NeedsReview,
    Skipped,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunEventKind {
    RunCreated,
    RunStarted,
    ImageQueued,
    ImageStarted,
    TaskStarted,
    TaskCompleted,
    TaskFailed,
    SkillResourceLoaded,
    ModelCallStarted,
    ModelCallCompleted,
    ModelCallFailed,
    ToolCallStarted,
    ToolCallCompleted,
    ValidationCompleted,
    RefinementStarted,
    RefinementCompleted,
    ArtifactCreated,
    ArtifactValidated,
    ArtifactCommitted,
    RetryScheduled,
    AnnotationDrafted,
    AnnotationCommitted,
    ReviewRequested,
    UsageUpdated,
    RunPaused,
    RunResumed,
    RunCancelled,
    RunBudgetExceeded,
    ImageCompleted,
    RunCompleted,
    RunFailed,
    RunInterrupted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RunEventPayload {
    State {
        from: Option<RunStatus>,
        to: RunStatus,
        reason: Option<String>,
    },
    Progress {
        completed_images: u64,
        total_images: u64,
        current_step: u32,
        max_steps: u32,
    },
    Message {
        summary: String,
    },
    Tool {
        call_id: ToolCallId,
        name: String,
        summary: String,
        success: bool,
    },
    Validation {
        issue_codes: Vec<String>,
        accepted: bool,
    },
    Annotation {
        annotation_ids: Vec<AnnotationId>,
        summary: String,
    },
    Artifact {
        artifact_ids: Vec<ArtifactId>,
        summary: String,
    },
    ProviderFailure {
        task_id: TaskId,
        node_id: String,
        provider: String,
        model: String,
        elapsed_ms: u64,
        retry_count: u32,
        error_code: String,
        summary: String,
    },
    TaskFailure {
        task_id: TaskId,
        node_id: String,
        elapsed_ms: u64,
        error_code: String,
        summary: String,
    },
    Usage {
        totals: UsageTotals,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunEvent {
    pub schema_version: u32,
    pub event_id: EventId,
    pub run_id: RunId,
    pub image_id: Option<ImageId>,
    pub task_id: Option<TaskId>,
    pub occurred_at: DateTime<Utc>,
    pub kind: RunEventKind,
    pub payload: RunEventPayload,
}

impl RunEvent {
    #[must_use]
    pub fn new(run_id: RunId, kind: RunEventKind, payload: RunEventPayload) -> Self {
        Self {
            schema_version: 1,
            event_id: EventId::new(),
            run_id,
            image_id: None,
            task_id: None,
            occurred_at: Utc::now(),
            kind,
            payload,
        }
    }

    #[must_use]
    pub fn scoped(mut self, image_id: Option<ImageId>, task_id: Option<TaskId>) -> Self {
        self.image_id = image_id;
        self.task_id = task_id;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states_cannot_restart() {
        assert!(!RunStatus::Completed.can_transition_to(RunStatus::Running));
        assert!(!RunStatus::Cancelled.can_transition_to(RunStatus::Paused));
        assert!(RunStatus::Paused.can_transition_to(RunStatus::Running));
        assert!(RunStatus::Running.can_transition_to(RunStatus::AwaitingReview));
        assert!(RunStatus::AwaitingReview.can_transition_to(RunStatus::Running));
    }

    #[test]
    fn event_round_trip_is_versioned() {
        let event = RunEvent::new(
            RunId::new(),
            RunEventKind::RunCreated,
            RunEventPayload::State {
                from: None,
                to: RunStatus::Pending,
                reason: None,
            },
        );
        let json = serde_json::to_string(&event).expect("serialize event");
        let decoded: RunEvent = serde_json::from_str(&json).expect("deserialize event");
        assert_eq!(decoded, event);
        assert_eq!(decoded.schema_version, 1);
    }
}
