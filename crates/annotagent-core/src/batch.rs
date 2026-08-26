//! Durable dataset-batch contracts and exact global budget accounting.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{ArtifactId, BatchId, ImageId, RunId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchStatus {
    Pending,
    Running,
    Paused,
    AwaitingReview,
    Completed,
    Partial,
    Failed,
    Cancelled,
    BudgetExceeded,
}

impl BatchStatus {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Partial | Self::Failed | Self::Cancelled | Self::BudgetExceeded
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchImageStatus {
    Pending,
    Leased,
    Running,
    AwaitingReview,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BatchUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub request_count: u64,
    pub image_count: u64,
    pub cost: Decimal,
}

impl BatchUsage {
    #[must_use]
    pub fn checked_add(&self, other: &Self) -> Option<Self> {
        Some(Self {
            input_tokens: self.input_tokens.checked_add(other.input_tokens)?,
            output_tokens: self.output_tokens.checked_add(other.output_tokens)?,
            total_tokens: self.total_tokens.checked_add(other.total_tokens)?,
            request_count: self.request_count.checked_add(other.request_count)?,
            image_count: self.image_count.checked_add(other.image_count)?,
            cost: self.cost.checked_add(other.cost)?,
        })
    }

    #[must_use]
    pub fn saturating_sub(&self, other: &Self) -> Self {
        Self {
            input_tokens: self.input_tokens.saturating_sub(other.input_tokens),
            output_tokens: self.output_tokens.saturating_sub(other.output_tokens),
            total_tokens: self.total_tokens.saturating_sub(other.total_tokens),
            request_count: self.request_count.saturating_sub(other.request_count),
            image_count: self.image_count.saturating_sub(other.image_count),
            cost: (self.cost - other.cost).max(Decimal::ZERO),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BatchBudgetLimits {
    pub max_input_tokens: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub max_total_tokens: Option<u64>,
    pub max_request_count: Option<u64>,
    pub max_image_count: Option<u64>,
    pub max_cost: Option<Decimal>,
    pub wall_clock_deadline: Option<DateTime<Utc>>,
}

impl BatchBudgetLimits {
    #[must_use]
    pub fn exceeded_by(&self, usage: &BatchUsage, now: DateTime<Utc>) -> Option<String> {
        let checks = [
            self.max_input_tokens
                .filter(|limit| usage.input_tokens > *limit)
                .map(|limit| format!("input token budget would exceed {limit}")),
            self.max_output_tokens
                .filter(|limit| usage.output_tokens > *limit)
                .map(|limit| format!("output token budget would exceed {limit}")),
            self.max_total_tokens
                .filter(|limit| usage.total_tokens > *limit)
                .map(|limit| format!("total token budget would exceed {limit}")),
            self.max_request_count
                .filter(|limit| usage.request_count > *limit)
                .map(|limit| format!("request budget would exceed {limit}")),
            self.max_image_count
                .filter(|limit| usage.image_count > *limit)
                .map(|limit| format!("image budget would exceed {limit}")),
        ];
        checks
            .into_iter()
            .flatten()
            .next()
            .or_else(|| {
                self.max_cost
                    .filter(|limit| usage.cost > *limit)
                    .map(|limit| format!("cost budget would exceed {limit}"))
            })
            .or_else(|| {
                self.wall_clock_deadline
                    .filter(|deadline| now >= *deadline)
                    .map(|deadline| format!("wall-clock deadline reached at {deadline}"))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BatchBudgetLedger {
    pub consumed: BatchUsage,
    pub reserved: BatchUsage,
}

impl BatchBudgetLedger {
    #[must_use]
    pub fn committed_and_reserved(&self) -> Option<BatchUsage> {
        self.consumed.checked_add(&self.reserved)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BatchNodeState {
    pub status: String,
    #[serde(default)]
    pub artifact_references: Vec<ArtifactId>,
    #[serde(default)]
    pub retry_count: u32,
    #[serde(default)]
    pub review_suspended: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BatchImageCheckpoint {
    #[serde(default)]
    pub node_states: BTreeMap<String, BatchNodeState>,
    #[serde(default)]
    pub artifact_references: Vec<ArtifactId>,
    #[serde(default)]
    pub retry_counters: BTreeMap<String, u32>,
    #[serde(default)]
    pub review_suspensions: BTreeSet<String>,
    pub runtime_checkpoint: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchRecord {
    pub id: BatchId,
    pub project_id: String,
    pub project_path: String,
    pub provider: String,
    pub status: BatchStatus,
    pub max_concurrency: u32,
    pub workflow_version: String,
    pub workflow_snapshot: serde_json::Value,
    pub project_snapshot: serde_json::Value,
    pub budget_limits: BatchBudgetLimits,
    pub budget_ledger: BatchBudgetLedger,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub event_sequence: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchImageRecord {
    pub batch_id: BatchId,
    pub image_id: ImageId,
    pub image_path: String,
    pub position: u64,
    pub status: BatchImageStatus,
    pub child_run_id: Option<RunId>,
    pub attempt_count: u32,
    pub reservation: BatchUsage,
    pub actual_usage: BatchUsage,
    pub checkpoint: BatchImageCheckpoint,
    pub error: Option<String>,
    pub lease_owner: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchEvent {
    pub batch_id: BatchId,
    pub sequence: u64,
    pub kind: String,
    pub image_id: Option<ImageId>,
    pub detail: serde_json::Value,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchCheckpoint {
    pub batch: BatchRecord,
    pub remaining_images: Vec<ImageId>,
    pub completed_images: Vec<ImageId>,
    pub current_node_states: BTreeMap<ImageId, BTreeMap<String, BatchNodeState>>,
    pub artifact_references: BTreeMap<ImageId, Vec<ArtifactId>>,
    pub retry_counters: BTreeMap<ImageId, BTreeMap<String, u32>>,
    pub review_suspensions: BTreeMap<ImageId, BTreeSet<String>>,
    pub event_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BatchProgress {
    pub total_images: u64,
    pub pending_images: u64,
    pub running_images: u64,
    pub completed_images: u64,
    pub failed_images: u64,
    pub review_images: u64,
    pub cancelled_images: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_combines_reserved_and_consumed_exactly() {
        let ledger = BatchBudgetLedger {
            consumed: BatchUsage {
                cost: Decimal::new(11, 2),
                request_count: 1,
                ..BatchUsage::default()
            },
            reserved: BatchUsage {
                cost: Decimal::new(7, 2),
                request_count: 2,
                ..BatchUsage::default()
            },
        };
        let total = ledger.committed_and_reserved().expect("bounded total");
        assert_eq!(total.cost, Decimal::new(18, 2));
        assert_eq!(total.request_count, 3);
    }
}
