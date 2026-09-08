//! Human-scoped, atomic sample corrections through the existing Sandbox and draft services.
use crate::LocalApplication;
use annotagent_core::ImageId;
use annotagent_storage::{
    ConversationImageClassAnswerInput, ConversationImageClassCreateInput,
    ConversationImageClassReview, ConversationImageClassScope, ConversationImageClassStatus,
};
use anyhow::{Context, Result, bail};
use uuid::Uuid;

/// Exact source for a separately authorized Builder repair, not a human-request alias.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationImageClassBuilderRepair {
    pub review_id: Uuid,
    pub draft_id: String,
    pub revision: u64,
    pub content_hash: String,
    pub schema_id: Uuid,
    pub schema_revision: u64,
    pub scope_digest: String,
    pub feedback_digest: String,
}

impl LocalApplication {
    pub fn conversation_image_class_builder_repair(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<ConversationImageClassBuilderRepair> {
        let review = self
            .conversation_image_class_review(project, conversation, task, id)?
            .context("Image-class review is unavailable in this task")?;
        if review.status != ConversationImageClassStatus::Applied || review.answer.is_none() {
            bail!(
                "Save all class decisions and finish local Draft preparation before requesting a Builder repair"
            );
        }
        let draft_id = review
            .repair_draft_id
            .as_deref()
            .context("Prepared repair Draft is unavailable")?;
        if draft_id != review.resume_checkpoint_ref.to_string()
            || review.scope.digest()? != review.scope_digest
        {
            bail!("The prepared repair source does not match the frozen class review");
        }
        let draft = self
            .store
            .available_image_class_repair_draft(project, draft_id)?;
        if draft.project_id != project
            || matches!(
                draft.status,
                annotagent_core::WorkflowDraftStatus::Published
                    | annotagent_core::WorkflowDraftStatus::Archived
            )
        {
            bail!("Class repair requires the task's editable prepared Draft");
        }
        let sample = self
            .store
            .get_workflow_sample_test_by_id(&review.scope.sample_test_id)?
            .context("Original sample is unavailable")?;
        if sample.project_id != project
            || sample.draft_id != review.scope.draft_id
            || sample.draft_revision != review.scope.draft_revision
            || sample.draft_content_hash != review.scope.draft_content_hash
            || !sample.report.sandbox
        {
            bail!("The original sample no longer matches this class review");
        }
        let seal = self
            .store
            .sample_scope_seal(&sample.id)?
            .context("This sample has no sealed Schema; a repair cannot guess the latest labels")?;
        let binding: annotagent_core::WorkflowSchemaBinding =
            serde_json::from_value(seal["annotation_schema"].clone())?;
        let schema_id = Uuid::parse_str(&binding.schema_draft_id)?;
        let schema = self.conversation_schema_draft(project, schema_id, Some(binding.revision))?;
        if schema.task_id != task
            || draft.annotation_schema.as_ref() != Some(&binding)
            || schema.definition.goal != binding.goal
            || schema.definition.task != binding.task
            || schema.definition.boundary_rules != binding.boundary_rules
        {
            bail!("Class repair must retain the exact tested task and sealed Schema");
        }
        let evidence = self
            .store
            .sample_plan_evidence(draft_id)?
            .context("Prepared class feedback evidence is unavailable")?;
        let feedback: Vec<annotagent_storage::SampleFeedbackRevision> =
            serde_json::from_value(evidence["feedback"].clone())?;
        let members = review
            .scope
            .members
            .iter()
            .map(|m| m.outcome.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = review
            .scope
            .baseline_feedback
            .iter()
            .filter(|r| {
                r.outcome_id
                    .as_deref()
                    .is_some_and(|outcome| members.contains(outcome))
            })
            .chain(review.revisions.iter())
            .cloned()
            .collect::<Vec<_>>();
        if evidence["project_id"] != project
            || evidence["sample_test_id"] != sample.id
            || feedback != expected
            || feedback.is_empty()
        {
            bail!(
                "Prepared evidence does not match the exact class decisions and their frozen baseline"
            );
        }
        Ok(ConversationImageClassBuilderRepair {
            review_id: id,
            draft_id: draft.id,
            revision: draft.revision,
            content_hash: draft.content_hash,
            schema_id,
            schema_revision: binding.revision,
            scope_digest: review.scope_digest,
            feedback_digest: annotagent_image_tools::sha256(&serde_json::to_vec(&feedback)?),
        })
    }
    fn validate_image_class_pixels(
        &self,
        project: &str,
        scope: &ConversationImageClassScope,
    ) -> Result<()> {
        let path = self.project_image_path(project, ImageId(Uuid::parse_str(&scope.image_id)?))?;
        if annotagent_image_tools::sha256(&std::fs::read(path)?) != scope.content_hash {
            bail!(
                "The image changed after this sample. No corrections were applied to different pixels"
            );
        }
        Ok(())
    }

    /// Preview only. Scope selection is not a human answer or an inference grant.
    pub fn conversation_image_class_scope(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        feedback: Uuid,
        target_label: Option<&str>,
    ) -> Result<ConversationImageClassScope> {
        let original = self
            .conversation_feedback_authorization(project, conversation, task, feedback)?
            .context("The original feedback authorization is unavailable")?;
        let current = self.conversation_feedback_context(
            project,
            conversation,
            task,
            original.consent.message_id,
        )?;
        if !current.matches_saved_context(&original.context)? {
            bail!(
                "The original feedback subject changed. A class review cannot silently adopt a new image or feedback baseline"
            );
        }
        let scope = self.store.conversation_image_class_scope(
            &self.conversation_project_identity(project)?,
            conversation,
            task,
            feedback,
            target_label,
        )?;
        self.validate_image_class_pixels(project, &scope)?;
        Ok(scope)
    }

    pub fn conversation_image_class_review(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<Option<ConversationImageClassReview>> {
        Ok(self.store.conversation_image_class_review(
            &self.conversation_project_identity(project)?,
            conversation,
            task,
            id,
        )?)
    }

    pub fn conversation_image_class_review_for_feedback(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        feedback: Uuid,
    ) -> Result<Option<ConversationImageClassReview>> {
        Ok(self.store.conversation_image_class_review_for_feedback(
            &self.conversation_project_identity(project)?,
            conversation,
            task,
            feedback,
        )?)
    }

    pub fn create_conversation_image_class_review(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        input: &ConversationImageClassCreateInput,
    ) -> Result<ConversationImageClassReview> {
        if let Some(saved) =
            self.conversation_image_class_review(project, conversation, task, input.id)?
        {
            if saved.input != *input {
                bail!("The original class review already has a different frozen scope");
            }
            return Ok(saved);
        }
        let scope = self.conversation_image_class_scope(
            project,
            conversation,
            task,
            input.feedback_call_id,
            input.target_label.as_deref(),
        )?;
        if scope.digest()? != input.expected_scope_digest {
            bail!(
                "The preview changed. Review the exact image and class before starting local edits"
            );
        }
        Ok(self.store.create_conversation_image_class_review(
            &self.conversation_project_identity(project)?,
            conversation,
            task,
            input,
        )?)
    }

    /// All members are saved together or none are saved. No Provider is involved.
    pub fn answer_conversation_image_class_review(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
        input: &ConversationImageClassAnswerInput,
    ) -> Result<ConversationImageClassReview> {
        let saved = self
            .conversation_image_class_review(project, conversation, task, id)?
            .context("This class review does not exist in the current task")?;
        if let Some(answer) = &saved.answer {
            if answer != input {
                bail!(
                    "The class review has a different saved answer. No corrections were replaced"
                );
            }
            return Ok(saved);
        }
        self.validate_image_class_pixels(project, &saved.scope)?;
        Ok(self.store.answer_conversation_image_class_review(
            &self.conversation_project_identity(project)?,
            conversation,
            task,
            id,
            input,
        )?)
    }

    pub fn cancel_conversation_image_class_review(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<ConversationImageClassReview> {
        Ok(self.store.cancel_conversation_image_class_review(
            &self.conversation_project_identity(project)?,
            conversation,
            task,
            id,
        )?)
    }

    /// One deterministic draft from this answer's exact revision IDs, not one per object.
    pub fn continue_conversation_image_class_review(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<ConversationImageClassReview> {
        let saved = self
            .conversation_image_class_review(project, conversation, task, id)?
            .context("This class review does not exist in the current task")?;
        if saved.status == ConversationImageClassStatus::Applied {
            return Ok(saved);
        }
        if saved.status != ConversationImageClassStatus::Answered {
            bail!("Save this class review's human answer before preparing a revised plan");
        }
        let owner = self.conversation_project_identity(project)?;
        let result = (|| {
            if self
                .store
                .sample_plan_evidence(&saved.resume_checkpoint_ref.to_string())?
                .is_none()
            {
                self.validate_image_class_pixels(project, &saved.scope)?;
            }
            Ok::<_, anyhow::Error>(self.store.resume_conversation_image_class_review(
                &owner,
                conversation,
                task,
                id,
            )?)
        })();
        if let Err(error) = result {
            self.store.record_conversation_image_class_resume_failure(
                &owner,
                conversation,
                task,
                id,
                &error.to_string(),
            )?;
        }
        self.conversation_image_class_review(project, conversation, task, id)?
            .context("Saved corrections are unavailable after resume")
    }

    pub(crate) fn recover_conversation_image_class_reviews(&self) -> Result<()> {
        for (owner, saved) in self.store.pending_conversation_image_class_resumes()? {
            let result = (|| {
                let sample = self
                    .store
                    .get_workflow_sample_test_by_id(&saved.scope.sample_test_id)?
                    .context("The class review's sample is unavailable for recovery")?;
                if self.conversation_project_identity(&sample.project_id)? != owner {
                    bail!("The class review's saved Project owner no longer matches its sample");
                }
                self.continue_conversation_image_class_review(
                    &sample.project_id,
                    saved.conversation_id,
                    saved.task_id,
                    saved.id,
                )
            })();
            if let Err(error) = result {
                self.store.record_conversation_image_class_resume_failure(
                    &owner,
                    saved.conversation_id,
                    saved.task_id,
                    saved.id,
                    &error.to_string(),
                )?;
            }
        }
        Ok(())
    }
}
