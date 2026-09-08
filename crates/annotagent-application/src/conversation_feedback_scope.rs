//! Human clarification of a saved candidate message, never an execution grant.
use crate::{ConversationFeedbackContext, ConversationFeedbackDecision, LocalApplication};
use annotagent_storage::{ConversationFeedbackScopeAnswer, ConversationFeedbackScopeAnswerInput};
use anyhow::{Context, Result, bail};
use uuid::Uuid;

impl LocalApplication {
    pub fn conversation_feedback_scope_answer(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        call: Uuid,
    ) -> Result<Option<ConversationFeedbackScopeAnswer>> {
        self.conversation_feedback_authorization(project, conversation, task, call)?
            .context("Feedback authorization not found in this task")?;
        Ok(self.store.conversation_feedback_scope_answer(
            &self.conversation_project_identity(project)?,
            task,
            call,
        )?)
    }

    /// Save a human's scope answer, without changing any candidate, Schema, grant,
    /// human correction request or resume event. Exact retry only restores history.
    pub fn answer_conversation_feedback_scope(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        call: Uuid,
        input: &ConversationFeedbackScopeAnswerInput,
    ) -> Result<ConversationFeedbackScopeAnswer> {
        let authorization = self
            .conversation_feedback_authorization(project, conversation, task, call)?
            .context("Feedback authorization not found in this task")?;
        let owner = self.conversation_project_identity(project)?;
        if let Some(saved) = self
            .store
            .conversation_feedback_scope_answer(&owner, task, call)?
        {
            if saved.input != *input {
                bail!(
                    "Scope clarification already has a different saved answer; no previous answer was overwritten"
                );
            }
            return Ok(saved);
        }
        let result = self
            .read_conversation_feedback(project, conversation, task, call)?
            .context("Feedback interpretation has not completed")?;
        if !matches!(
            result
                .decision
                .map_err(|message| anyhow::anyhow!(message))?,
            ConversationFeedbackDecision::ClarifyScope { .. }
        ) {
            bail!("Only a completed scope clarification accepts this answer");
        }
        let context: ConversationFeedbackContext =
            serde_json::from_value(authorization.context.clone())?;
        if result
            .receipt
            .evidence
            .as_ref()
            .context("Feedback evidence is unavailable")?["context"]["subject"]
            != authorization.context
            || input.expected_context_digest
                != annotagent_storage::conversation_feedback_context_digest(&authorization.context)?
            || self.conversation_feedback_context(
                project,
                conversation,
                task,
                authorization.consent.message_id,
            )? != context
        {
            bail!(
                "The original feedback subject or revision changed; your scope answer was not applied to a different object"
            );
        }
        Ok(self.store.answer_conversation_feedback_scope(
            &owner,
            conversation,
            task,
            call,
            input,
            &result.receipt,
        )?)
    }
}
