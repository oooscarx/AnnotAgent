//! Versioned events and run-state transitions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{AnnotationId, EventId, ImageId, RunId, TaskId, ToolCallId, UsageTotals};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    Paused,
    AwaitingReview,
    Completed,
    Cancelled,
    BudgetExceeded,
    Failed,
}

impl RunStatus {
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        use RunStatus::{
            AwaitingReview, BudgetExceeded, Cancelled, Completed, Failed, Paused, Pending, Running,
        };
        matches!(
            (self, next),
            (Pending | Paused, Running | Cancelled)
                | (
                    Running,
                    Paused | AwaitingReview | Completed | Cancelled | BudgetExceeded | Failed
                )
                | (AwaitingReview, Running | Completed | Cancelled)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunEventKind {
    RunCreated,
    RunStarted,
    ImageQueued,
    ImageStarted,
    TaskStarted,
    SkillResourceLoaded,
    ModelCallStarted,
    ModelCallCompleted,
    ToolCallStarted,
    ToolCallCompleted,
    ValidationCompleted,
    RefinementStarted,
    RefinementCompleted,
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
