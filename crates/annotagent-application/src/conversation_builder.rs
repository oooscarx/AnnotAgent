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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_class_repair: Option<crate::ConversationImageClassBuilderRepair>,
}

pub use annotagent_storage::ConversationBuilderRepair;

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
    pub fn conversation_builder_operation(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<Option<ConversationBuilderOperation>> {
        self.optional_conversation_builder_budget(project, conversation, task)?;
        Ok(self.store.conversation_builder_operation(
            &self.conversation_project_identity(project)?,
            task,
            id,
        )?)
    }
    /// Read-only preview of a delivered correction's existing repair copy.
    pub fn conversation_builder_repair(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        request_id: Uuid,
    ) -> Result<ConversationBuilderRepair> {
        let request = self.store.conversation_human_request(
            &self.conversation_project_identity(project)?,
            request_id,
        )?;
        if request.input.conversation_id != conversation || request.input.task_id != task {
            bail!("Human request belongs to another task or is unavailable");
        }
        if request.status != annotagent_storage::ConversationHumanRequestStatus::Applied {
            bail!("Submit the correction and finish local Draft preparation before repair");
        }
        let id = request
            .resume_draft_id
            .ok_or_else(|| anyhow!("Repair Draft is unavailable"))?;
        let draft = self
            .store
            .available_conversation_repair_draft(project, &id)?;
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
        self.conversation_builder_history_scoped(project, conversation, task, None, None, None)
    }
    pub fn conversation_builder_history_scoped(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        operation: Option<Uuid>,
        image_class_review: Option<Uuid>,
        human_request: Option<Uuid>,
    ) -> Result<serde_json::Value> {
        if [operation, image_class_review, human_request]
            .iter()
            .flatten()
            .count()
            > 1
        {
            bail!("Choose an exact operation or a repair source, not both");
        }
        if !self
            .conversation_tasks(project, conversation)?
            .iter()
            .any(|item| item.input.id == task)
        {
            bail!("Task belongs to another conversation");
        }
        let owner = self.conversation_project_identity(project)?;
        let operations = if let Some(operation) = operation {
            self.store
                .conversation_builder_operation(&owner, task, operation)?
                .into_iter()
                .collect()
        } else if let Some(review) = image_class_review {
            self.conversation_image_class_review(project, conversation, task, review)?
                .ok_or_else(|| {
                    anyhow!("Image-class review belongs to another task or is unavailable")
                })?;
            self.store
                .conversation_image_class_builder_history(&owner, task, review)?
        } else if let Some(request_id) = human_request {
            let request = self.store.conversation_human_request(&owner, request_id)?;
            if request.input.conversation_id != conversation || request.input.task_id != task {
                bail!("Human request belongs to another task or is unavailable");
            }
            self.store.conversation_human_builder_history(
                &owner,
                task,
                request_id,
                request.resume_draft_id.as_deref(),
            )?
        } else {
            self.store.conversation_builder_history(&owner, task)?
        };
        let items = operations
            .into_iter()
            .map(|operation| {
                let session = self
                    .store
                    .get_agent_session(operation.id)
                    .ok()
                    .filter(|session| session.project_id.as_deref() == Some(project));
                let draft_id = session.as_ref().and_then(|session| session.working_draft.as_ref()).map_or_else(|| operation.id.to_string(), |draft| draft.draft_id.clone());
                // A revision is meaningful only together with its Schema identity.
                // Prefer the operation's frozen pair: later edits to the working
                // Draft cannot relabel what this Builder actually consumed.
                let identity = operation.evidence.as_ref().and_then(|evidence| {
                    let id = Uuid::parse_str(evidence.get("schema_id")?.as_str()?).ok()?;
                    let revision = evidence.get("schema_revision")?.as_u64()?;
                    (!id.is_nil() && revision > 0).then_some((id, revision))
                }).or_else(|| self.store.get_workflow_draft(&draft_id).ok()
                    .filter(|draft|draft.project_id == project)
                    .and_then(|draft|draft.annotation_schema)
                    .and_then(|binding| Some((Uuid::parse_str(&binding.schema_draft_id).ok()?, binding.revision)))
                );
                serde_json::json!({"operation":operation,"session":session,"schema_id":identity.map(|(id,_)|id),"schema_revision":identity.map(|(_,revision)|revision)})
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
        if execution.repair.is_some() && execution.image_class_repair.is_some() {
            bail!("A Builder operation must have one exact repair source, not two");
        }
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
        let source = if let Some(repair) = &execution.image_class_repair {
            serde_json::json!({"kind":"image_class_review","reference":repair})
        } else if let Some(repair) = &execution.repair {
            serde_json::json!({"kind":"human_request","reference":repair})
        } else {
            serde_json::Value::Null
        };
        if let Some(saved) = self.store.reserve_conversation_builder_with_source(
            &owner,
            execution.task_id,
            execution.operation_id,
            &hash,
            (execution.schema_id, execution.schema_revision),
            &source,
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
            let draft = self
                .store
                .available_conversation_repair_draft(project, &actual.draft_id)?;
            if draft.revision != expected.revision || draft.content_hash != expected.content_hash {
                bail!("Repair Draft changed while loading the authorized revision");
            }
            Some(draft)
        } else if let Some(expected) = &execution.image_class_repair {
            let actual = self.conversation_image_class_builder_repair(
                project,
                execution.conversation_id,
                execution.task_id,
                expected.review_id,
            )?;
            if actual != *expected
                || expected.schema_id != execution.schema_id
                || expected.schema_revision != execution.schema_revision
            {
                bail!("Image-class repair source or Schema changed; review a fresh authorization");
            }
            let draft = self
                .store
                .available_conversation_repair_draft(project, &expected.draft_id)?;
            if draft.revision != expected.revision || draft.content_hash != expected.content_hash {
                bail!("Image-class repair Draft changed while loading the authorized revision");
            }
            Some(draft)
        } else {
            None
        };
        let constraints = WorkflowConstraints::default();
        let mut input = self.workflow_advisor_input(project, settings, constraints.clone())?;
        if let Some(journey) = self.store.conversation_journey_for_builder(
            &owner,
            execution.conversation_id,
            execution.task_id,
            execution.operation_id,
        )? {
            if journey.revoked || journey.consent.expires_at <= chrono::Utc::now() {
                bail!("Journey authorization was revoked or expired");
            }
            let allowed = journey
                .effective_consent()
                .allowed_models
                .iter()
                .map(|model| model.model_id.as_str())
                .collect::<std::collections::BTreeSet<_>>();
            input
                .model_profiles
                .retain(|model| allowed.contains(format!("model-profile:{}", model.id).as_str()));
            input.expert_models.retain(|model| {
                allowed.contains(model.model_id.as_str())
                    || match &model.connection {
                        annotagent_core::ModelConnection::ProviderModel {
                            provider_id,
                            remote_model_id,
                        } => input.model_profiles.iter().any(|profile| {
                            profile.provider_id == *provider_id
                                && profile.remote_model_id == *remote_model_id
                        }),
                        _ => false,
                    }
            });
            input.model_registry.retain(|model| {
                allowed.contains(model.id.as_str())
                    || input.model_profiles.iter().any(|profile| {
                        profile.remote_model_id == model.id
                            || profile.remote_model_id == model.model
                    })
            });
            input.provider_profiles.retain(|provider| {
                input
                    .model_profiles
                    .iter()
                    .any(|model| model.provider_id == provider.id)
            });
        }
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
            let review = pipeline
                .steps
                .iter()
                .find(|step| step.node_type == "core.human_review")
                .unwrap();
            assert_eq!(
                review.parameters.get("task_id"),
                Some(&serde_json::json!(pipeline.target_task_id))
            );
            assert_eq!(
                review.parameters.get("target_label"),
                Some(&serde_json::json!(pipeline.target_label))
            );
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
