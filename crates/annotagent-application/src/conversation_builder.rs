//! Thin orchestration of the existing Builder; no model calls or publishing on read.
use crate::{LocalApplication, PipelineBuilderModelRuntime, Settings};
use annotagent_core::{
    ModelBinding as PipelineModelBinding, ModelCapability, PipelineBuildMode,
    PipelineBuilderConstraints, ProviderAdapterKind, RegistryWorkflowAdvisor, VisionCapability,
    VisionModelProvider, WorkflowAdvisor, WorkflowConstraints, WorkflowSchemaBinding,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued_plan: Option<QueuedWorkflowSource>,
}

pub use annotagent_storage::ConversationBuilderRepair;

/// Exact editable-plan source for an ordinary queued supplement. Unlike human
/// repair provenance, this does not claim a correction or quality measurement.
pub use annotagent_storage::QueuedWorkflowSource;

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
    /// Passive source selection: no copy, call, draft edit or inferred active task.
    pub fn queued_workflow_source(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        message: Uuid,
        draft_id: &str,
    ) -> Result<QueuedWorkflowSource> {
        let queued = self.queued_conversation_message(project, conversation, task, message)?;
        if queued.cancelled_at.is_some() {
            bail!("Queued instruction is cancelled");
        }
        let draft = self
            .store
            .available_conversation_repair_draft(project, draft_id)?;
        if draft.project_id != project
            || matches!(
                draft.status,
                annotagent_core::WorkflowDraftStatus::Published
                    | annotagent_core::WorkflowDraftStatus::Archived
            )
        {
            bail!(
                "Select this task's editable Workflow Draft; published versions require the existing explicit improvement flow"
            );
        }
        let binding = draft
            .annotation_schema
            .as_ref()
            .ok_or_else(|| anyhow!("The selected plan has no stable conversation Schema owner"))?;
        let schema_id = Uuid::parse_str(&binding.schema_draft_id)?;
        let schema = self.conversation_schema_draft(project, schema_id, Some(binding.revision))?;
        if schema.task_id != task
            || schema.definition.task != binding.task
            || schema.definition.goal != binding.goal
            || schema.definition.boundary_rules != binding.boundary_rules
        {
            bail!("Selected plan belongs to another task or its Schema binding changed");
        }
        let evidence = self.store.sample_plan_evidence(draft_id)?;
        if evidence
            .as_ref()
            .is_some_and(|value| value["project_id"] != project)
        {
            bail!("Saved plan evidence belongs to another Project");
        }
        Ok(QueuedWorkflowSource {
            message_id: message,
            draft_id: draft.id,
            revision: draft.revision,
            content_hash: draft.content_hash,
            schema_id,
            schema_revision: binding.revision,
            evidence_hash: evidence
                .map(|value| {
                    serde_json::to_vec(&value).map(|bytes| annotagent_image_tools::sha256(&bytes))
                })
                .transpose()?,
        })
    }

    /// Revalidate after approval before using the preserved nodes, bindings and
    /// policies. Never silently pick a newer Draft or strip its saved evidence.
    pub fn load_queued_workflow_source(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        source: &QueuedWorkflowSource,
    ) -> Result<annotagent_core::WorkflowDraft> {
        let current = self.queued_workflow_source(
            project,
            conversation,
            task,
            source.message_id,
            &source.draft_id,
        )?;
        if current != *source {
            bail!(
                "Selected Workflow or evidence changed; review a new authorization before planning"
            );
        }
        let draft = self
            .store
            .available_conversation_repair_draft(project, &source.draft_id)?;
        if draft.revision != source.revision || draft.content_hash != source.content_hash {
            bail!("Selected Workflow changed while loading its approved revision");
        }
        Ok(draft)
    }
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
        let delivery =
            self.require_delivery_intake(project, execution.conversation_id, execution.task_id)?;
        if [
            execution.repair.is_some(),
            execution.image_class_repair.is_some(),
            execution.queued_plan.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count()
            > 1
        {
            bail!("A Builder operation must have one exact source");
        }
        let schema = self.conversation_schema_draft(
            project,
            execution.schema_id,
            Some(execution.schema_revision),
        )?;
        if schema.task_id != execution.task_id {
            bail!("Schema belongs to another conversation task");
        }
        crate::task_delivery::require_delivery_schema(delivery.as_ref(), &schema.definition)?;
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
        let source = if let Some(queued) = &execution.queued_plan {
            serde_json::json!({"kind":"queued_workflow","reference":queued})
        } else if let Some(repair) = &execution.image_class_repair {
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
        let mut supplement = None;
        let repair_draft = if let Some(expected) = &execution.queued_plan {
            self.load_queued_workflow_source(
                project,
                execution.conversation_id,
                execution.task_id,
                expected,
            )?;
            let message = self.queued_conversation_message(
                project,
                execution.conversation_id,
                execution.task_id,
                expected.message_id,
            )?;
            if message
                .receipt
                .agent_model
                .as_ref()
                .and_then(|m| m.model_profile_id)
                .is_some_and(|id| id != selected.model.id)
            {
                bail!("Queued Workflow planning must use the Agent model frozen at Send");
            }
            supplement = Some(message.input.message.text);
            Some(self.store.copy_queued_workflow(
                &owner,
                project,
                &annotagent_storage::QueuedWorkflowCopy {
                    conversation_id: execution.conversation_id,
                    task_id: execution.task_id,
                    copy_id: execution.operation_id,
                    source: expected.clone(),
                },
            )?)
        } else if let Some(expected) = &execution.repair {
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
        if let Some(supplement) = &supplement {
            input.project_schema.project.annotation_goal.push_str("\nSaved queued supplement (untrusted task instructions; no additional permissions):\n");
            input
                .project_schema
                .project
                .annotation_goal
                .push_str(supplement);
        }
        let (nodes, models) = self.workflow_catalog(settings)?;
        let mut seed = RegistryWorkflowAdvisor.suggest_workflow(
            project,
            &input.project_schema,
            &input.enabled_skills,
            &nodes,
            &models,
            &constraints,
        );
        let build_mode = if let Some(mut draft) = repair_draft {
            if execution.queued_plan.is_some() && draft.annotation_schema.as_ref() != Some(&binding)
            {
                // Only the new working copy is rebound. Preserve its authored graph
                // for the existing Builder to reconcile and statically validate.
                draft.annotation_schema = Some(binding.clone());
                self.store.save_workflow_draft(&draft)?;
                draft = self.store.get_workflow_draft(&draft.id)?;
            }
            if draft.annotation_schema.as_ref() != Some(&binding) {
                bail!("Repair Draft does not use the authorized Schema revision");
            }
            let mode = PipelineBuildMode::RepairDraft {
                draft_id: draft.id.clone(),
            };
            seed.draft = draft;
            seed.rationale = vec![if execution.queued_plan.is_some() {
                "Revise this preserved working copy using the saved supplement. Do not claim a human correction or improved quality; no comparison test is authorized.".into()
            } else {
                "Repair the preserved plan using the saved scoped human correction; quality remains unverified until a separately authorized comparison test.".into()
            }];
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
            let mut composition =
                conversation_composition(&input.project_schema, &binding, &constraints, &models)?;
            prefer_registered_vlm_detection(&mut composition, &input);
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
            planning_only: true,
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

/// Select the registered structured VLM detection operation when an exact ready
/// VLM Profile is available but no exact ready object-detector Profile exists.
/// The annotation kind determines the output Artifact, not a hard-coded model
/// family; the existing Registry declarations determine the executable route.
fn prefer_registered_vlm_detection(
    composition: &mut annotagent_core::LabelWorkflowComposition,
    input: &annotagent_core::WorkflowAdvisorInput,
) -> bool {
    let non_fixture = |profile: &&annotagent_core::ModelProfile| {
        input
            .provider_profiles
            .iter()
            .find(|provider| provider.id == profile.provider_id)
            .is_some_and(|provider| provider.adapter != ProviderAdapterKind::Mock)
    };
    if crate::compatible_builder_models(input, Some(ModelCapability::ObjectDetection))
        .iter()
        .any(non_fixture)
    {
        return false;
    }
    let mut profiles =
        crate::compatible_builder_models(input, Some(ModelCapability::VisionLanguage))
            .into_iter()
            .filter(non_fixture)
            .collect::<Vec<_>>();
    profiles.sort_by_key(|profile| profile.id.to_string());
    let Some(profile) = profiles.first() else {
        return false;
    };
    let mut changed = false;
    for step in composition
        .shared_stages
        .iter_mut()
        .flat_map(|stage| &mut stage.steps)
    {
        if step.node_type != annotagent_skill_object_detection::OBJECT_DETECTION_OPERATION
            && step.node_type != "capability.detect"
        {
            continue;
        }
        annotagent_skill_vlm_detection::VLM_DETECTION_OPERATION.clone_into(&mut step.node_type);
        step.kind = annotagent_core::WorkflowNodeKind::VisionLanguageModel;
        step.model_binding = Some(PipelineModelBinding {
            model_id: profile.remote_model_id.clone(),
            capability: VisionCapability::VisionLanguage,
            configuration: std::collections::BTreeMap::new(),
        });
        let labels = step
            .parameters
            .get("target_labels")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        step.parameters.insert("labels".to_owned(), labels);
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    struct QueueTestProvider(std::sync::Mutex<Vec<annotagent_core::ModelRequest>>);
    #[async_trait::async_trait]
    impl VisionModelProvider for QueueTestProvider {
        fn name(&self) -> &str {
            "TEST queued Builder"
        }
        fn capabilities(&self) -> annotagent_core::ModelCapabilities {
            annotagent_core::ModelCapabilities {
                vision: false,
                tool_calls: true,
                json_schema: true,
                usage_reporting: true,
                multi_image: false,
            }
        }
        async fn complete(
            &self,
            request: annotagent_core::ModelRequest,
            _: CancellationToken,
        ) -> annotagent_core::CoreResult<annotagent_core::ModelResponse> {
            self.0.lock().unwrap().push(request);
            Ok(annotagent_core::ModelResponse {
                content: None,
                tool_calls: vec![annotagent_core::ModelToolCall {
                    id: Uuid::new_v4().to_string().into(),
                    name: "inspect_project".into(),
                    arguments: serde_json::json!({}),
                }],
                usage: annotagent_core::TokenUsage::known(
                    10,
                    5,
                    annotagent_core::UsageSource::Mock,
                ),
                request_id: Some("TEST local request".into()),
                provider_metadata: std::collections::BTreeMap::new(),
            })
        }
    }
    #[tokio::test]
    async fn queued_workflow_source_preserves_manual_plan_and_rejects_retargeting() {
        use annotagent_storage::{
            ConversationMessageInput, ConversationSendInput, ConversationSendMode,
        };
        let temp = tempfile::tempdir().unwrap();
        let app = LocalApplication::new(temp.path()).unwrap();
        let project = "TEST-queued-workflow-source";
        app.create_project(project,"version: 1\nproject:\n  name: TEST source\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n").unwrap();
        let conversation = app.create_project_conversation(project).unwrap();
        let owner = app.conversation_project_identity(project).unwrap();
        let mut message = ConversationSendInput {
            message: ConversationMessageInput {
                id: Uuid::new_v4(),
                text: "TEST find cups".into(),
                image: None,
                reference: None,
            },
            task_id: None,
            schema_revision: app.project_goal(project).unwrap()["revision"]
                .as_str()
                .unwrap()
                .into(),
            mode: Some(ConversationSendMode::Plan),
            agent_model: None,
        };
        let root = app
            .store
            .send_conversation_message(&owner, conversation, &message)
            .unwrap();
        let task = root.task_id;
        let decision = crate::ConversationSchemaDecision::Draft {
            delivery: None,
            kind: crate::ConversationOutputKind::BoundingBox,
            labels: vec!["cup".into()],
            multi_label: false,
            attributes: std::collections::BTreeMap::new(),
            boundary_rules: vec!["TEST preserve exclusions".into()],
            rationale: "TEST human definition".into(),
        };
        let schema = app
            .save_human_conversation_schema_draft(
                project,
                conversation,
                task,
                Uuid::new_v4(),
                &decision,
            )
            .unwrap();
        message.task_id = Some(task);
        message.message.id = Uuid::new_v4();
        message.message.text = "TEST refine boundaries without replacing the plan".into();
        app.store
            .send_conversation_message(&owner, conversation, &message)
            .unwrap();
        let binding = WorkflowSchemaBinding {
            schema_draft_id: schema.id.to_string(),
            revision: schema.revision,
            goal: schema.definition.goal,
            task: schema.definition.task,
            boundary_rules: schema.definition.boundary_rules,
        };
        let draft:annotagent_core::WorkflowDraft=serde_json::from_value(serde_json::json!({"id":"TEST-preserved-plan","project_id":project,"name":"TEST manual plan","status":"editing","annotation_schema":binding,"nodes":[{"id":"image","node_type":"core.image_input","kind":"image_input","parameters":{"TEST_manual_note":"keep"}}],"runtime_policies":{"TEST_manual_policy":{"keep":true}},"created_at":chrono::Utc::now(),"updated_at":chrono::Utc::now()})).unwrap();
        app.store.save_workflow_draft(&draft).unwrap();
        let original = app.store.get_workflow_draft(&draft.id).unwrap();
        let source = app
            .queued_workflow_source(project, conversation, task, message.message.id, &draft.id)
            .unwrap();
        assert_eq!(
            app.load_queued_workflow_source(project, conversation, task, &source)
                .unwrap(),
            original
        );
        assert!(
            app.queued_workflow_source(
                project,
                conversation,
                Uuid::new_v4(),
                message.message.id,
                &draft.id
            )
            .is_err()
        );
        let mut invalid = source.clone();
        invalid.evidence_hash = Some("a".repeat(64));
        assert!(
            app.load_queued_workflow_source(project, conversation, task, &invalid)
                .is_err()
        );
        assert!(
            app.optional_conversation_builder_budget(project, conversation, task)
                .unwrap()
                .is_none()
        );
        drop(app);
        let app = LocalApplication::new(temp.path()).unwrap();
        assert_eq!(
            app.load_queued_workflow_source(project, conversation, task, &source)
                .unwrap(),
            original
        );
        let mut edited = original.clone();
        let copy_request = annotagent_storage::QueuedWorkflowCopy {
            conversation_id: conversation,
            task_id: task,
            copy_id: Uuid::new_v4(),
            source: source.clone(),
        };
        let copied = app
            .store
            .copy_queued_workflow(&owner, project, &copy_request)
            .unwrap();
        assert_eq!(copied.nodes, original.nodes);
        assert_eq!(copied.runtime_policies, original.runtime_policies);
        assert_eq!(copied.annotation_schema, original.annotation_schema);
        assert_eq!(
            app.store.get_workflow_draft(&original.id).unwrap(),
            original
        );
        assert!(
            app.store
                .copy_queued_workflow("foreign", project, &copy_request)
                .is_err()
        );
        let mut manual_copy = copied.clone();
        manual_copy.name = "TEST edited working copy".into();
        app.store.save_workflow_draft(&manual_copy).unwrap();
        let saved_copy = app.store.get_workflow_draft(&copied.id).unwrap();
        assert_eq!(
            app.store
                .copy_queued_workflow(&owner, project, &copy_request)
                .unwrap(),
            saved_copy
        );
        let mut retargeted = copy_request.clone();
        retargeted.source.content_hash = "b".repeat(64);
        assert!(
            app.store
                .copy_queued_workflow(&owner, project, &retargeted)
                .is_err()
        );
        edited.name = "TEST manually revised after approval".into();
        app.store.save_workflow_draft(&edited).unwrap();
        assert!(
            app.load_queued_workflow_source(project, conversation, task, &source)
                .is_err()
        );
        let updated = app
            .queued_workflow_source(project, conversation, task, message.message.id, &draft.id)
            .unwrap();
        assert_ne!(updated.content_hash, source.content_hash);
        assert_eq!(
            app.store
                .copy_queued_workflow(&owner, project, &copy_request)
                .unwrap(),
            saved_copy,
            "retry must not recopy a newer source"
        );
        let another = annotagent_storage::QueuedWorkflowCopy {
            copy_id: Uuid::new_v4(),
            ..copy_request.clone()
        };
        assert!(
            app.store
                .copy_queued_workflow(&owner, project, &another)
                .is_err()
        );
        app.cancel_project_queued_message(project, conversation, task, message.message.id)
            .unwrap();
        assert!(
            app.load_queued_workflow_source(project, conversation, task, &updated)
                .is_err()
        );
        let cancelled = annotagent_storage::QueuedWorkflowCopy {
            source: updated,
            ..another
        };
        assert!(
            app.store
                .copy_queued_workflow(&owner, project, &cancelled)
                .is_err()
        );
        drop(app);
        let app = LocalApplication::new(temp.path()).unwrap();
        assert_eq!(
            app.store
                .copy_queued_workflow(&owner, project, &copy_request)
                .unwrap(),
            saved_copy
        );
        message.message.id = Uuid::new_v4();
        message.message.text = "TEST keep manual nodes and consider local verification".into();
        app.store
            .send_conversation_message(&owner, conversation, &message)
            .unwrap();
        let selected = crate::tests::register_pipeline_builder_model(&app, "TEST queued model");
        let source = app
            .queued_workflow_source(
                project,
                conversation,
                task,
                message.message.id,
                &original.id,
            )
            .unwrap();
        let original_before = app.store.get_workflow_draft(&original.id).unwrap();
        let operation = Uuid::new_v4();
        let grant = annotagent_storage::ConversationCallGrant {
            id: operation,
            task_id: task,
            scope_hash: "e".repeat(64),
            maximum_calls: 2,
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(10),
        };
        app.store
            .authorize_conversation_calls(&owner, &grant)
            .unwrap();
        let execution = ConversationBuilderExecution {
            conversation_id: conversation,
            task_id: task,
            schema_id: source.schema_id,
            schema_revision: source.schema_revision,
            operation_id: operation,
            scope_hash: grant.scope_hash.clone(),
            repair: None,
            image_class_repair: None,
            queued_plan: Some(source),
        };
        let provider = QueueTestProvider(std::sync::Mutex::new(Vec::new()));
        let settings = crate::load_settings(None).unwrap();
        let outcome = app
            .build_conversation_pipeline(
                project,
                &execution,
                &settings,
                &selected,
                &provider,
                CancellationToken::default(),
            )
            .await
            .unwrap();
        assert_eq!(outcome.status, "completed");
        assert_eq!(
            outcome.evidence.as_ref().unwrap()["repair_source"]["kind"],
            "queued_workflow"
        );
        assert_eq!(outcome.evidence.as_ref().unwrap()["published"], false);
        assert_eq!(outcome.evidence.as_ref().unwrap()["samples_tested"], false);
        let copy = app
            .store
            .get_workflow_draft(&operation.to_string())
            .unwrap();
        assert_eq!(copy.nodes, original_before.nodes);
        assert_eq!(copy.runtime_policies, original_before.runtime_policies);
        assert_eq!(
            app.store.get_workflow_draft(&original.id).unwrap(),
            original_before
        );
        let count =
            {
                let calls = provider.0.lock().unwrap();
                assert!(!calls.is_empty());
                assert!(calls.len() <= 2);
                assert!(
                    calls[0]
                        .messages
                        .iter()
                        .any(|m| m.content.contains(&message.message.text))
                );
                assert!(calls.iter().all(|r| r.images.is_empty()
                    && !r.tools.iter().any(|t| t.name == "dry_run_pipeline")));
                calls.len()
            };
        assert_eq!(
            app.build_conversation_pipeline(
                project,
                &execution,
                &settings,
                &selected,
                &provider,
                CancellationToken::default()
            )
            .await
            .unwrap(),
            outcome
        );
        assert_eq!(provider.0.lock().unwrap().len(), count);
    }

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
        let mut classification_project = project.clone();
        classification_project.tasks[0].kind = annotagent_core::TaskKind::Classification;
        let classification = crate::controlled_label_composition(
            &classification_project,
            "objects",
            "cup",
            &WorkflowConstraints::default(),
            &models,
        )
        .unwrap();
        let review = classification.label_pipelines[0]
            .steps
            .iter()
            .find(|step| step.kind == annotagent_core::WorkflowNodeKind::HumanReview)
            .unwrap();
        assert_eq!(
            review.parameters.get("task_id"),
            Some(&serde_json::json!("objects"))
        );
        assert_eq!(
            review.parameters.get("target_label"),
            Some(&serde_json::json!("cup"))
        );
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
        let mut reinforced = draft.clone();
        crate::add_mandatory_geometry_review_boundaries(&mut reinforced).unwrap();
        for commit in reinforced
            .nodes
            .iter()
            .filter(|node| node.kind == annotagent_core::WorkflowNodeKind::Commit)
        {
            let boundaries = reinforced
                .nodes
                .iter()
                .filter(|node| {
                    node.kind == annotagent_core::WorkflowNodeKind::HumanReview
                        && reinforced
                            .edges
                            .iter()
                            .any(|edge| edge.from_node == node.id && edge.to_node == commit.id)
                })
                .collect::<Vec<_>>();
            assert!(!boundaries.is_empty());
            assert!(commit.parameters.contains_key("task_id"));
            for review in boundaries {
                for key in ["task_id", "target_label"] {
                    assert_eq!(
                        review.parameters.get(key),
                        commit.parameters.get(key),
                        "inserted boundary must preserve {key}"
                    );
                }
            }
        }
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

    #[test]
    fn bounding_box_conversation_uses_ready_vlm_when_no_detector_profile_exists() {
        let temporary = tempfile::tempdir().unwrap();
        let app = LocalApplication::new(temporary.path()).unwrap();
        let project_id = "TEST-conversation-vlm-route";
        let project: annotagent_core::ProjectSchema = serde_yaml::from_str("version: 1\nproject:\n  name: TEST conversation VLM route\ndataset:\n  root: images\nruntime: {}\ntasks:\n  - id: objects\n    kind: bounding_box\n    labels: [cup]\n    required: true\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n").unwrap();
        app.create_project(project_id, &serde_yaml::to_string(&project).unwrap())
            .unwrap();
        let selected = crate::tests::register_pipeline_builder_model(&app, "TEST shared VLM");
        let mut provider = app
            .store
            .get_provider_profile(selected.provider.id)
            .unwrap();
        provider.adapter = ProviderAdapterKind::OpenAiCompatible;
        provider.credential_ref = Some(annotagent_core::CredentialReference {
            provider_id: provider.id,
            source: annotagent_core::CredentialSource::EnvironmentVariable,
            locator: "TEST_CONVERSATION_VLM_KEY".into(),
        });
        provider.health.status = annotagent_core::ProviderHealthStatus::Available;
        app.store.save_provider_profile(&provider).unwrap();
        let mut model = selected.model.clone();
        model.id = annotagent_core::ModelProfileId::new();
        model.display_name = "TEST ready visual route".into();
        model.remote_model_id = "TEST-ready-vlm-route".into();
        model
            .input_modalities
            .insert(annotagent_core::InputModality::Image);
        model
            .task_capabilities
            .insert(ModelCapability::VisionLanguage);
        app.store.save_model_profile(&model).unwrap();
        let settings = crate::load_settings(None).unwrap();
        let input = app
            .workflow_advisor_input_for_label(
                project_id,
                &settings,
                WorkflowConstraints::default(),
                None,
                None,
            )
            .unwrap();
        let binding = WorkflowSchemaBinding {
            schema_draft_id: Uuid::new_v4().to_string(),
            revision: 1,
            goal: "Find cups using the registered visual capability".into(),
            task: project.tasks[0].clone(),
            boundary_rules: vec!["Exclude logos".into()],
        };
        let (_, runtime_models) = app.workflow_catalog(&settings).unwrap();
        let mut composition = conversation_composition(
            &project,
            &binding,
            &WorkflowConstraints::default(),
            &runtime_models,
        )
        .unwrap();
        assert!(prefer_registered_vlm_detection(&mut composition, &input));
        let shared_id = composition.shared_stages[0].steps[0].id.clone();
        let shared = &composition.shared_stages[0].steps[0];
        assert_eq!(
            shared.node_type,
            annotagent_skill_vlm_detection::VLM_DETECTION_OPERATION
        );
        assert_eq!(
            shared.kind,
            annotagent_core::WorkflowNodeKind::VisionLanguageModel
        );
        assert_eq!(
            shared.model_binding.as_ref().unwrap().capability,
            VisionCapability::VisionLanguage
        );
        let mut draft = composition.compile_draft(
            project_id,
            "TEST VLM route",
            std::collections::BTreeMap::new(),
            chrono::Utc::now(),
        );
        crate::bind_available_registry_models(&mut draft, &input);
        let detector = draft
            .nodes
            .iter()
            .find(|node| node.id == shared_id)
            .unwrap();
        assert_eq!(
            detector
                .model_profile_binding
                .as_ref()
                .map(|binding| binding.model_profile_id),
            Some(model.id)
        );
        assert!(!prefer_registered_vlm_detection(&mut composition, &input));
    }
}
