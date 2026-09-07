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
    }
}
impl LocalApplication {
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
