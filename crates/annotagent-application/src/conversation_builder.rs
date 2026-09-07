//! Thin orchestration of the existing Builder; no model calls or publishing on read.
use crate::{LocalApplication, PipelineBuilderModelRuntime, Settings};
use annotagent_core::{
    PipelineBuildMode, PipelineBuilderConstraints, RegistryWorkflowAdvisor, VisionModelProvider,
    WorkflowAdvisor, WorkflowConstraints, WorkflowSchemaBinding,
};
use annotagent_storage::ConversationBuilderOperation;
use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationBuilderExecution {
    pub conversation_id: Uuid,
    pub task_id: Uuid,
    pub schema_id: Uuid,
    pub schema_revision: u64,
    pub operation_id: Uuid,
    pub scope_hash: String,
}

struct BuilderGuard<'a> {
    app: &'a LocalApplication,
    owner: String,
    task: Uuid,
    id: Uuid,
}
impl Drop for BuilderGuard<'_> {
    fn drop(&mut self) {
        let _ = self.app.store.settle_conversation_builder(&self.owner,self.task,self.id,false,&serde_json::json!({"error":"Builder handler ended; saved Draft and receipts remain. No automatic retry."}));
        if let Ok(mut calls) = self.app.conversation_cancellations.lock() {
            if let Some(token) = calls.remove(&self.id) {
                token.cancel();
            }
        }
    }
}
impl LocalApplication {
    pub fn conversation_builder_grant(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<annotagent_storage::ConversationCallGrant> {
        self.conversation_builder_budget(project, conversation, task)?;
        let owner = self.conversation_project_identity(project)?;
        Ok(self.store.conversation_authorization(&owner, task, id)?)
    }
    pub fn conversation_builder_budget(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<annotagent_storage::ConversationCallBudget> {
        if !self
            .conversation_tasks(project, conversation)?
            .iter()
            .any(|item| item.input.id == task)
        {
            bail!("Task belongs to another conversation");
        }
        let owner = self.conversation_project_identity(project)?;
        self.store
            .conversation_call_budget(&owner, task)?
            .ok_or_else(|| anyhow!("Task authorization not found"))
    }
    pub fn advance_conversation_builder_authorization(
        &self,
        project: &str,
        conversation: Uuid,
        previous: Uuid,
        grant: &annotagent_storage::ConversationCallGrant,
    ) -> Result<()> {
        self.conversation_builder_budget(project, conversation, grant.task_id)?;
        let owner = self.conversation_project_identity(project)?;
        self.store
            .advance_conversation_authorization(&owner, previous, grant)?;
        Ok(())
    }
    pub fn conversation_builder_history(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<serde_json::Value> {
        if !self
            .conversation_tasks(project, conversation)?
            .iter()
            .any(|item| item.input.id == task)
        {
            bail!("Task belongs to another conversation");
        }
        let owner = self.conversation_project_identity(project)?;
        let operations = self.store.conversation_builder_history(&owner, task)?;
        let items = operations
            .into_iter()
            .map(|operation| {
                let session = self
                    .store
                    .get_agent_session(operation.id)
                    .ok()
                    .filter(|session| session.project_id.as_deref() == Some(project));
                serde_json::json!({"operation":operation,"session":session})
            })
            .collect::<Vec<_>>();
        Ok(serde_json::json!({"items":items}))
    }
    pub async fn build_conversation_pipeline(
        &self,
        project: &str,
        execution: &ConversationBuilderExecution,
        settings: &Settings,
        selected: &PipelineBuilderModelRuntime,
        provider: &dyn VisionModelProvider,
        cancellation: CancellationToken,
    ) -> Result<ConversationBuilderOperation> {
        let schema = self.conversation_schema_draft(
            project,
            execution.schema_id,
            Some(execution.schema_revision),
        )?;
        if schema.task_id != execution.task_id {
            bail!("Schema belongs to another conversation task");
        }
        if !self
            .conversation_tasks(project, execution.conversation_id)?
            .iter()
            .any(|task| task.input.id == execution.task_id)
        {
            bail!("Builder task belongs to another conversation");
        }
        let owner = self.conversation_project_identity(project)?;
        let hash = annotagent_image_tools::sha256(&serde_json::to_vec(
            &serde_json::json!({"execution":execution,"model":selected.safe_selection(),"settings":settings}),
        )?);
        if let Some(saved) = self.store.reserve_conversation_builder(
            &owner,
            execution.task_id,
            execution.operation_id,
            &hash,
        )? {
            return Ok(saved);
        }
        let _guard = BuilderGuard {
            app: self,
            owner: owner.clone(),
            task: execution.task_id,
            id: execution.operation_id,
        };
        self.conversation_cancellations
            .lock()
            .map_err(|_| anyhow!("Cancellation registry unavailable"))?
            .insert(execution.operation_id, cancellation.clone());
        if self
            .store
            .conversation_call_cancellations(&owner, execution.task_id)?
            .iter()
            .any(|intent| intent.call_id == execution.operation_id)
        {
            cancellation.cancel();
        }
        let metered = self.conversation_text_provider(
            project,
            execution.conversation_id,
            execution.task_id,
            &execution.scope_hash,
            &selected.model.remote_model_id,
            provider,
        )?;
        if cancellation.is_cancelled() {
            bail!("Builder cancelled before execution");
        }
        let constraints = WorkflowConstraints::default();
        let mut input = self.workflow_advisor_input(project, settings, constraints.clone())?;
        let binding = WorkflowSchemaBinding {
            schema_draft_id: schema.id.to_string(),
            revision: schema.revision,
            goal: schema.definition.goal,
            task: schema.definition.task,
            boundary_rules: schema.definition.boundary_rules,
        };
        binding.apply_to(&mut input.project_schema);
        let (nodes, models) = self.workflow_catalog(settings)?;
        let mut seed = RegistryWorkflowAdvisor.suggest_workflow(
            project,
            &input.project_schema,
            &input.enabled_skills,
            &nodes,
            &models,
            &constraints,
        );
        seed.draft.annotation_schema = Some(binding);
        let budget = self
            .store
            .conversation_call_budget(&owner, execution.task_id)?
            .ok_or_else(|| anyhow!("Task authorization disappeared"))?;
        let remaining = budget
            .current_grant
            .maximum_calls
            .saturating_sub(budget.used_calls);
        if remaining == 0 {
            bail!("Task call allowance exhausted; Builder was not started");
        }
        let limits = PipelineBuilderConstraints {
            maximum_agent_turns: remaining.min(16),
            maximum_dry_runs: 0,
            ..PipelineBuilderConstraints::default()
        };
        let report = self
            .run_workflow_advisor_loop(
                project,
                settings,
                &constraints,
                None,
                input,
                seed,
                &metered,
                Some(selected),
                None,
                limits,
                PipelineBuildMode::FromScratch,
                cancellation,
                Some(execution.operation_id),
            )
            .await?;
        self.store.settle_conversation_builder(&owner,execution.task_id,execution.operation_id,true,&serde_json::json!({"session_id":report.session.id,"draft_id":report.suggestion.as_ref().map(|suggestion|&suggestion.draft.id),"outcome":report.session.outcome,"published":false,"samples_tested":false}))?;
        self.store
            .conversation_builder_operation(&owner, execution.task_id, execution.operation_id)?
            .ok_or_else(|| anyhow!("Builder receipt missing"))
    }
}
