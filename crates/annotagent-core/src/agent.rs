//! Shared, auditable session state for bounded AnnotAgent loops.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
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
    }
}
