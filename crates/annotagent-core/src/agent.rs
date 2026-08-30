//! Shared, auditable session state for bounded `AnnotAgent` loops.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DETECTION_RECOVERY_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    PipelineBuilder,
    WorkflowAdvisor,
    AnnotationRecovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSessionStatus {
    Running,
    WaitingForHuman,
    Succeeded,
    Failed,
    BudgetExceeded,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentBudget {
    pub max_steps: u32,
    pub max_tool_calls: u32,
    pub max_tokens: Option<u64>,
    pub max_cost: Option<Decimal>,
}

impl Default for AgentBudget {
    fn default() -> Self {
        Self {
            max_steps: 16,
            max_tool_calls: 16,
            max_tokens: None,
            max_cost: None,
        }
    }
}

impl AgentBudget {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_steps == 0 {
            return Err("Agent max_steps must be greater than zero".to_owned());
        }
        if self.max_tool_calls == 0 {
            return Err("Agent max_tool_calls must be greater than zero".to_owned());
        }
        if self.max_cost.is_some_and(|value| value < Decimal::ZERO) {
            return Err("Agent max_cost cannot be negative".to_owned());
        }
        Ok(())
    }

    #[must_use]
    pub fn can_reserve(&self, usage: &AgentUsage, tool_calls: u32, cost: Decimal) -> bool {
        usage.steps.saturating_add(tool_calls) <= self.max_steps
            && usage.tool_calls.saturating_add(tool_calls) <= self.max_tool_calls
            && self.max_cost.is_none_or(|limit| usage.cost + cost <= limit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectionFallbackQuery {
    pub id: String,
    pub text: String,
    pub target_label: crate::LabelId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DetectionRecoveryPolicy {
    pub allow_fallback: bool,
    pub max_fallback_calls: u32,
    pub fallback_estimated_cost: Decimal,
    pub match_minimum_iou: f32,
    pub initial_gate: crate::EvidenceGateConfig,
    pub final_gate: crate::EvidenceGateConfig,
}

impl Default for DetectionRecoveryPolicy {
    fn default() -> Self {
        Self {
            allow_fallback: true,
            max_fallback_calls: 1,
            fallback_estimated_cost: Decimal::ZERO,
            match_minimum_iou: 0.6,
            initial_gate: crate::EvidenceGateConfig {
                accept_when: vec![crate::EvidenceAcceptRule {
                    minimum_score: Some(0.85),
                    no_domain_issue: true,
                    ..crate::EvidenceAcceptRule::default()
                }],
                fallback_when: vec![
                    crate::EvidenceFallbackRule {
                        empty_specialist_result: true,
                        ..crate::EvidenceFallbackRule::default()
                    },
                    crate::EvidenceFallbackRule {
                        specialist_score_below: Some(0.55),
                        ..crate::EvidenceFallbackRule::default()
                    },
                    crate::EvidenceFallbackRule {
                        domain_issue: true,
                        ..crate::EvidenceFallbackRule::default()
                    },
                    crate::EvidenceFallbackRule {
                        correction_risk_above: Some(0.7),
                        ..crate::EvidenceFallbackRule::default()
                    },
                ],
                review_when: vec![crate::EvidenceReviewRule {
                    score_missing: true,
                    ..crate::EvidenceReviewRule::default()
                }],
                reject_when: Vec::new(),
            },
            final_gate: crate::EvidenceGateConfig {
                accept_when: vec![crate::EvidenceAcceptRule {
                    minimum_sources: Some(2),
                    minimum_iou: Some(0.6),
                    no_domain_issue: true,
                    ..crate::EvidenceAcceptRule::default()
                }],
                fallback_when: Vec::new(),
                review_when: vec![
                    crate::EvidenceReviewRule {
                        geometry_conflict: true,
                        ..crate::EvidenceReviewRule::default()
                    },
                    crate::EvidenceReviewRule {
                        label_conflict: true,
                        ..crate::EvidenceReviewRule::default()
                    },
                    crate::EvidenceReviewRule {
                        open_vocab_only: true,
                        ..crate::EvidenceReviewRule::default()
                    },
                    crate::EvidenceReviewRule {
                        empty_result: true,
                        ..crate::EvidenceReviewRule::default()
                    },
                ],
                reject_when: Vec::new(),
            },
        }
    }
}

impl DetectionRecoveryPolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.fallback_estimated_cost < Decimal::ZERO {
            return Err("fallback_estimated_cost cannot be negative".to_owned());
        }
        if self.max_fallback_calls > 1 {
            return Err("Detection Recovery Alpha permits at most one fallback call".to_owned());
        }
        if !(0.0..=1.0).contains(&self.match_minimum_iou) {
            return Err("match_minimum_iou must be within [0,1]".to_owned());
        }
        self.initial_gate.validate()?;
        self.final_gate.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectionRecoveryRequest {
    #[serde(default = "default_detection_recovery_protocol_version")]
    pub protocol_version: u32,
    #[serde(default)]
    pub policy: DetectionRecoveryPolicy,
    #[serde(default)]
    pub budget: AgentBudget,
    #[serde(default)]
    pub queries: Vec<DetectionFallbackQuery>,
}

const fn default_detection_recovery_protocol_version() -> u32 {
    DETECTION_RECOVERY_PROTOCOL_VERSION
}

impl DetectionRecoveryRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.protocol_version != DETECTION_RECOVERY_PROTOCOL_VERSION {
            return Err(format!(
                "unsupported Detection Recovery protocol version {}",
                self.protocol_version
            ));
        }
        self.policy.validate()?;
        self.budget.validate()?;
        let mut query_ids = std::collections::BTreeSet::new();
        for query in &self.queries {
            if query.id.trim().is_empty()
                || query.text.trim().is_empty()
                || query.target_label.as_str().trim().is_empty()
            {
                return Err(
                    "Detection fallback queries require id, text, and target_label".to_owned(),
                );
            }
            if !query_ids.insert(query.id.as_str()) {
                return Err(format!("duplicate Detection fallback query {:?}", query.id));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionRecoveryAction {
    KeepPrimary,
    InvokeFallback,
    HumanReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionRecoveryStopCondition {
    PrimaryAccepted,
    InitialReviewRequired,
    FallbackCompleted,
    FallbackDisabled,
    BudgetInsufficient,
    FallbackUnavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectionRecoveryReport {
    pub protocol_version: u32,
    pub action: DetectionRecoveryAction,
    pub initial_evidence: crate::EvidenceGateReport,
    pub final_evidence: crate::EvidenceGateReport,
    pub fallback_model_id: Option<crate::ModelId>,
    pub fallback_invoked: bool,
    pub fallback_call_count: u32,
    pub stop_condition: DetectionRecoveryStopCondition,
    pub session: AgentSession,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AgentUsage {
    pub steps: u32,
    pub tool_calls: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: Decimal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentToolStep {
    pub sequence: u32,
    pub call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub result: serde_json::Value,
    pub success: bool,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: Uuid,
    pub project_id: Option<String>,
    pub run_id: Option<crate::RunId>,
    pub kind: AgentKind,
    pub status: AgentSessionStatus,
    pub budget: AgentBudget,
    pub usage: AgentUsage,
    pub steps: Vec<AgentToolStep>,
    pub stop_reason: Option<String>,
    pub pending_human_action: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AgentSession {
    #[must_use]
    pub fn start(kind: AgentKind, budget: AgentBudget) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            project_id: None,
            run_id: None,
            kind,
            status: AgentSessionStatus::Running,
            budget,
            usage: AgentUsage::default(),
            steps: Vec::new(),
            stop_reason: None,
            pending_human_action: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[must_use]
    pub fn with_project(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = Some(project_id.into());
        self
    }

    #[must_use]
    pub fn with_run(mut self, run_id: crate::RunId) -> Self {
        self.run_id = Some(run_id);
        self
    }

    pub fn record_tool(
        &mut self,
        tool_name: impl Into<String>,
        arguments: serde_json::Value,
        result: serde_json::Value,
        success: bool,
    ) -> Result<(), String> {
        if self.status != AgentSessionStatus::Running {
            return Err("cannot record a tool after the Agent session stopped".to_owned());
        }
        if self.usage.steps >= self.budget.max_steps
            || self.usage.tool_calls >= self.budget.max_tool_calls
        {
            self.stop_budget("step or tool-call budget exhausted");
            return Err("Agent budget exhausted".to_owned());
        }
        let now = Utc::now();
        self.usage.steps += 1;
        self.usage.tool_calls += 1;
        self.steps.push(AgentToolStep {
            sequence: self.usage.steps,
            call_id: format!("{}:{}", self.id, self.usage.steps),
            tool_name: tool_name.into(),
            arguments,
            result,
            success,
            started_at: now,
            finished_at: Utc::now(),
        });
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn add_model_usage(&mut self, input_tokens: u64, output_tokens: u64, cost: Decimal) {
        self.usage.input_tokens = self.usage.input_tokens.saturating_add(input_tokens);
        self.usage.output_tokens = self.usage.output_tokens.saturating_add(output_tokens);
        self.usage.cost += cost;
        let total = self
            .usage
            .input_tokens
            .saturating_add(self.usage.output_tokens);
        if self.budget.max_tokens.is_some_and(|limit| total > limit)
            || self
                .budget
                .max_cost
                .is_some_and(|limit| self.usage.cost > limit)
        {
            self.stop_budget("token or cost budget exhausted");
        }
    }

    pub fn wait_for_human(&mut self, action: impl Into<String>) {
        self.status = AgentSessionStatus::WaitingForHuman;
        self.pending_human_action = Some(action.into());
        self.stop_reason = Some("explicit human approval is required".to_owned());
        self.updated_at = Utc::now();
    }

    pub fn cancel(&mut self) {
        self.status = AgentSessionStatus::Cancelled;
        self.pending_human_action = None;
        self.stop_reason = Some("cancelled by operator".to_owned());
        self.updated_at = Utc::now();
    }

    pub fn fail(&mut self, reason: impl Into<String>) {
        self.status = AgentSessionStatus::Failed;
        self.stop_reason = Some(reason.into());
        self.updated_at = Utc::now();
    }

    pub fn succeed(&mut self, reason: impl Into<String>) {
        self.status = AgentSessionStatus::Succeeded;
        self.stop_reason = Some(reason.into());
        self.updated_at = Utc::now();
    }

    fn stop_budget(&mut self, reason: &str) {
        self.status = AgentSessionStatus::BudgetExceeded;
        self.stop_reason = Some(reason.to_owned());
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_enforces_tool_budget_and_human_stop() {
        let mut session = AgentSession::start(
            AgentKind::WorkflowAdvisor,
            AgentBudget {
                max_steps: 1,
                max_tool_calls: 1,
                ..AgentBudget::default()
            },
        );
        session
            .record_tool(
                "inspect",
                serde_json::json!({}),
                serde_json::json!({}),
                true,
            )
            .expect("first tool");
        assert!(
            session
                .record_tool("again", serde_json::json!({}), serde_json::json!({}), true)
                .is_err()
        );
        assert_eq!(session.status, AgentSessionStatus::BudgetExceeded);

        let mut approval = AgentSession::start(AgentKind::WorkflowAdvisor, AgentBudget::default());
        approval.wait_for_human("publish_workflow");
        assert_eq!(approval.status, AgentSessionStatus::WaitingForHuman);
        approval.cancel();
        assert_eq!(approval.status, AgentSessionStatus::Cancelled);
        assert!(approval.pending_human_action.is_none());

        let policy = DetectionRecoveryPolicy {
            max_fallback_calls: 2,
            ..DetectionRecoveryPolicy::default()
        };
        assert!(
            policy
                .validate()
                .expect_err("Alpha fallback limit")
                .contains("at most one")
        );
    }
}
