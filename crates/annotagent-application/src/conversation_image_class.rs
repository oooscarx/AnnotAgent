//! Human-scoped, atomic sample corrections through the existing Sandbox and draft services.
use crate::LocalApplication;
use annotagent_core::ImageId;
use annotagent_storage::{
    ConversationImageClassAnswerInput, ConversationImageClassCreateInput,
    ConversationImageClassReview, ConversationImageClassScope, ConversationImageClassStatus,
};
use anyhow::{Context, Result, bail};
use uuid::Uuid;

impl LocalApplication {
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
