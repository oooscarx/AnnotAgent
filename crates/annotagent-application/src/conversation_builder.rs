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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair: Option<ConversationBuilderRepair>,
}

/// Exact editable copy approved for repair, never a mutable pointer to the latest plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationBuilderRepair {
    pub request_id: Uuid,
    pub draft_id: String,
    pub revision: u64,
    pub content_hash: String,
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
    /// Read-only preview of a delivered correction's existing repair copy.
    pub fn conversation_builder_repair(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        request_id: Uuid,
    ) -> Result<ConversationBuilderRepair> {
        let request = self
            .conversation_human_requests(project, conversation, task)?
            .into_iter()
            .find(|request| request.input.id == request_id)
            .ok_or_else(|| anyhow!("Human request belongs to another task or is unavailable"))?;
        if request.status != annotagent_storage::ConversationHumanRequestStatus::Applied {
            bail!("Submit the correction and finish local Draft preparation before repair");
        }
        let id = request
            .resume_draft_id
            .ok_or_else(|| anyhow!("Repair Draft is unavailable"))?;
        let draft = self.store.get_workflow_draft(&id)?;
        if draft.project_id != project
            || id != request.input.resume_checkpoint_ref.to_string()
            || matches!(
                draft.status,
                annotagent_core::WorkflowDraftStatus::Published
                    | annotagent_core::WorkflowDraftStatus::Archived
            )
        {
            bail!("Repair requires this task's editable prepared Draft");
        }
        let evidence = self
            .store
            .sample_plan_evidence(&id)?
            .ok_or_else(|| anyhow!("Repair feedback is unavailable"))?;
        let feedback: Vec<annotagent_storage::SampleFeedbackRevision> =
            serde_json::from_value(evidence["feedback"].clone())?;
        if evidence["project_id"] != project
            || evidence["sample_test_id"] != request.input.sample_test_id
            || request
                .answer
                .as_ref()
                .is_none_or(|answer| feedback.as_slice() != [answer.clone()])
        {
            bail!("Repair evidence no longer matches the submitted correction");
        }
        Ok(ConversationBuilderRepair {
            request_id,
            draft_id: id,
            revision: draft.revision,
            content_hash: draft.content_hash,
        })
    }
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
        self.optional_conversation_builder_budget(project, conversation, task)?
            .ok_or_else(|| anyhow!("Task authorization not found"))
    }
    pub fn optional_conversation_builder_budget(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<Option<annotagent_storage::ConversationCallBudget>> {
        if !self
            .conversation_tasks(project, conversation)?
            .iter()
            .any(|item| item.input.id == task)
        {
            bail!("Task belongs to another conversation");
        }
        let owner = self.conversation_project_identity(project)?;
        Ok(self.store.conversation_call_budget(&owner, task)?)
    }
    pub fn initial_conversation_builder_authorization(
        &self,
        project: &str,
        conversation: Uuid,
        grant: &annotagent_storage::ConversationCallGrant,
    ) -> Result<()> {
        self.optional_conversation_builder_budget(project, conversation, grant.task_id)?;
        let owner = self.conversation_project_identity(project)?;
        self.store.authorize_conversation_calls(&owner, grant)?;
        Ok(())
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
                let draft_id = session.as_ref().and_then(|session| session.working_draft.as_ref()).map_or_else(|| operation.id.to_string(), |draft| draft.draft_id.clone());
                let schema_revision=self.store.get_workflow_draft(&draft_id).ok().filter(|draft|draft.project_id==project).and_then(|draft|draft.annotation_schema.map(|binding|binding.revision));
                serde_json::json!({"operation":operation,"session":session,"schema_revision":schema_revision})
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
        let repair_draft = if let Some(expected) = &execution.repair {
            let actual = self.conversation_builder_repair(
                project,
                execution.conversation_id,
                execution.task_id,
                expected.request_id,
            )?;
            if &actual != expected {
                bail!(
                    "Repair Draft changed; review the current revision before authorizing another call"
                );
            }
            let draft = self.store.get_workflow_draft(&actual.draft_id)?;
            if draft.revision != expected.revision || draft.content_hash != expected.content_hash {
                bail!("Repair Draft changed while loading the authorized revision");
            }
            Some(draft)
        } else {
            None
        };
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
        let build_mode = if let Some(draft) = repair_draft {
            if draft.annotation_schema.as_ref() != Some(&binding) {
                bail!("Repair Draft does not use the authorized Schema revision");
            }
            let mode = PipelineBuildMode::RepairDraft {
                draft_id: draft.id.clone(),
            };
            seed.draft = draft;
            seed.rationale = vec!["Repair the preserved plan using the saved scoped human correction; quality remains unverified until a separately authorized comparison test.".into()];
            seed.estimated_model_calls_per_image = seed
                .draft
                .nodes
                .iter()
                .filter(|node| node.model_binding.is_some() || node.model_profile_binding.is_some())
                .count();
            seed.estimated_latency_ms = None;
            seed.estimated_cost_tier = "unresolved".into();
            seed.unresolved_model_bindings = seed
                .draft
                .nodes
                .iter()
                .filter_map(|node| {
                    node.unresolved_model_requirement
                        .as_ref()
                        .map(|requirement| requirement.reason.clone())
                })
                .collect();
            seed.warnings.clear();
            seed.alternatives.clear();
            mode
        } else {
            let composition =
                conversation_composition(&input.project_schema, &binding, &constraints, &models)?;
            seed.draft = composition.compile_draft(
                project,
                "Conversation annotation plan",
                input.project_schema.project.enabled_skill_versions(),
                chrono::Utc::now(),
            );
            crate::bind_available_registry_models(&mut seed.draft, &input);
            seed.draft.annotation_schema = Some(binding);
            PipelineBuildMode::FromScratch
        };
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
                build_mode,
                cancellation,
                Some(execution.operation_id),
            )
            .await?;
        self.store.settle_conversation_builder(&owner,execution.task_id,execution.operation_id,true,&serde_json::json!({"session_id":report.session.id,"draft_id":report.suggestion.as_ref().map(|suggestion|&suggestion.draft.id),"draft_revision":report.suggestion.as_ref().map(|suggestion|suggestion.draft.revision),"draft_content_hash":report.suggestion.as_ref().map(|suggestion|&suggestion.draft.content_hash),"outcome":report.session.outcome,"schema_id":execution.schema_id,"schema_revision":execution.schema_revision,"published":false,"samples_tested":false}))?;
        self.store
            .conversation_builder_operation(&owner, execution.task_id, execution.operation_id)?
            .ok_or_else(|| anyhow!("Builder receipt missing"))
    }
}

/// Reuse the controlled Skill/Core grammar and preserve all Schema labels.
/// Detection routes share the same model configuration; the geometry review gate stays intact.
fn conversation_composition(
    project: &annotagent_core::ProjectSchema,
    binding: &WorkflowSchemaBinding,
    constraints: &annotagent_core::WorkflowConstraints,
    models: &annotagent_core::ModelRegistry,
) -> Result<annotagent_core::LabelWorkflowComposition> {
    let first = binding
        .task
        .labels
        .first()
        .ok_or_else(|| anyhow!("Schema has no labels"))?;
    let mut result = crate::controlled_label_composition(
        project,
        binding.task.id.as_str(),
        first,
        constraints,
        models,
    )?;
    if binding.task.kind == annotagent_core::TaskKind::Classification {
        return Ok(result);
    }
    if binding.task.kind != annotagent_core::TaskKind::BoundingBox {
        bail!("Conversation execution supports classification and bounding boxes");
    }
    let normalize = |composition: &mut annotagent_core::LabelWorkflowComposition| {
        for step in composition
            .shared_stages
            .iter_mut()
            .flat_map(|stage| &mut stage.steps)
        {
            for key in ["labels", "target_labels"] {
                if step.parameters.contains_key(key) {
                    step.parameters
                        .insert(key.into(), serde_json::json!(binding.task.labels));
                }
            }
            if step.parameters.contains_key("target_description") {
                step.parameters.insert(
                    "target_description".into(),
                    serde_json::json!(format!(
                        "{}\n{}",
                        binding.goal,
                        binding.boundary_rules.join("\n")
                    )),
                );
            }
            if step.parameters.contains_key("class_mapping") {
                step.parameters.insert(
                    "class_mapping".into(),
                    serde_json::json!(
                        binding
                            .task
                            .labels
                            .iter()
                            .map(|label| (label, label))
                            .collect::<std::collections::BTreeMap<_, _>>()
                    ),
                );
            }
            if step.parameters.contains_key("queries") {
                step.parameters.insert("queries".into(), serde_json::json!(binding.task.labels.iter().map(|label| serde_json::json!({"id":label,"text":label.replace(['_','-']," "),"target_label":label})).collect::<Vec<_>>()));
            }
        }
    };
    normalize(&mut result);
    for label in binding.task.labels.iter().skip(1) {
        let mut route = crate::controlled_label_composition(
            project,
            binding.task.id.as_str(),
            label,
            constraints,
            models,
        )?;
        normalize(&mut route);
        if route.shared_stages != result.shared_stages {
            bail!("Label routes require incompatible shared model configurations");
        }
        result.label_pipelines.extend(route.label_pipelines);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounding_box_labels_share_one_detector_and_keep_each_geometry_review_route() {
        let project: annotagent_core::ProjectSchema = serde_yaml::from_str("version: 1\nproject:\n  name: TEST shared conversation detector\ndataset:\n  root: images\nruntime: {}\ntasks:\n  - id: objects\n    kind: bounding_box\n    labels: [cup, can, plate]\n    required: true\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n").unwrap();
        let binding = WorkflowSchemaBinding {
            schema_draft_id: Uuid::new_v4().to_string(),
            revision: 1,
            goal: "Find cups, cans and plates, not bottles".into(),
            task: project.tasks[0].clone(),
            boundary_rules: vec!["Exclude bottles".into()],
        };
        let temporary = tempfile::tempdir().unwrap();
        let app = LocalApplication::new(temporary.path()).unwrap();
        let (_, models) = app
            .workflow_catalog(&crate::load_settings(None).unwrap())
            .unwrap();
        let composition =
            conversation_composition(&project, &binding, &WorkflowConstraints::default(), &models)
                .unwrap();
        assert_eq!(composition.shared_stages.len(), 1);
        assert_eq!(composition.shared_stages[0].steps.len(), 1);
        assert_eq!(composition.label_pipelines.len(), 3);
        let shared = &composition.shared_stages[0].steps[0];
        let labels = shared
            .parameters
            .get("labels")
            .or_else(|| shared.parameters.get("target_labels"))
            .unwrap();
        assert_eq!(labels, &serde_json::json!(["cup", "can", "plate"]));
        for (pipeline, label) in composition.label_pipelines.iter().zip(&binding.task.labels) {
            assert_eq!(pipeline.target_label.as_str(), label);
            assert!(
                pipeline
                    .steps
                    .iter()
                    .any(|step| step.node_type == "core.human_review" && step.review_gate.required)
            );
            assert!(
                pipeline
                    .steps
                    .iter()
                    .all(|step| step.model_binding.is_none())
            );
        }
        let draft = composition.compile_draft(
            "TEST",
            "TEST shared detector",
            project.project.enabled_skill_versions(),
            chrono::Utc::now(),
        );
        assert_eq!(
            draft
                .nodes
                .iter()
                .filter(|node| node.model_binding.is_some())
                .count(),
            1
        );
        assert_eq!(
            draft
                .nodes
                .iter()
                .filter(|node| node.node_type == "core.human_review")
                .count(),
            3
        );
    }
}
