//! Durable feedback scope, layered on the existing shared call ledger.
use crate::{ConversationFeedbackContext, LocalApplication};
use annotagent_storage::ConversationFeedbackAuthorizationRecord;
use anyhow::{Context, Result, bail};
use uuid::Uuid;

impl LocalApplication {
    pub fn conversation_feedback_authorization(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        call: Uuid,
    ) -> Result<Option<ConversationFeedbackAuthorizationRecord>> {
        self.optional_conversation_builder_budget(project, conversation, task)?;
        Ok(self.store.conversation_feedback_authorization(
            &self.conversation_project_identity(project)?,
            task,
            call,
        )?)
    }

    pub fn conversation_feedback_for_message(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        message: Uuid,
    ) -> Result<Option<ConversationFeedbackAuthorizationRecord>> {
        self.optional_conversation_builder_budget(project, conversation, task)?;
        Ok(self.store.conversation_feedback_for_message(
            &self.conversation_project_identity(project)?,
            task,
            message,
        )?)
    }

    /// Explicitly confirmed scope only. No Provider dispatch, annotation edits or
    /// human answers. The Store commits the consent and cumulative grant together.
    pub fn authorize_conversation_feedback(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        record: &ConversationFeedbackAuthorizationRecord,
    ) -> Result<ConversationFeedbackAuthorizationRecord> {
        self.optional_conversation_builder_budget(project, conversation, task)?;
        let owner = self.conversation_project_identity(project)?;
        if let Some(saved) =
            self.store
                .conversation_feedback_authorization(&owner, task, record.consent.call_id)?
        {
            if &saved != record {
                bail!("Feedback authorization retry changed its saved scope");
            }
            return Ok(saved);
        }
        let context: ConversationFeedbackContext = serde_json::from_value(record.context.clone())?;
        if context.message.input.id != record.consent.message_id
            || self.conversation_feedback_context(
                project,
                conversation,
                task,
                record.consent.message_id,
            )? != context
        {
            bail!("Feedback subject changed before authorization; no model request was sent");
        }
        Ok(self
            .store
            .authorize_conversation_feedback(&owner, conversation, task, record)?)
    }

    /// Read-only view also works after restart, when an interrupted call has only
    /// a generic unknown-outcome receipt and no response evidence.
    pub fn conversation_feedback_status(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        call: Uuid,
    ) -> Result<serde_json::Value> {
        let authorization = self
            .conversation_feedback_authorization(project, conversation, task, call)?
            .context("Feedback authorization not found in this task")?;
        let receipt = self.conversation_call_receipt(project, conversation, task, call)?;
        let cancelled = self
            .conversation_schema_cancellations(project, conversation, task)?
            .iter()
            .any(|item| item.call_id == call);
        let mut decision = serde_json::Value::Null;
        let mut error = None;
        if let Some(receipt) = &receipt {
            match receipt.status {
                annotagent_storage::ConversationCallStatus::Completed => {
                    match self.read_conversation_feedback(project, conversation, task, call) {
                        Ok(Some(result)) => decision = serde_json::to_value(result.decision)?,
                        Ok(None) => error = Some("Saved feedback receipt is missing".to_string()),
                        Err(problem) => error = Some(problem.to_string()),
                    }
                }
                annotagent_storage::ConversationCallStatus::Reserved => {}
                annotagent_storage::ConversationCallStatus::InDoubt => {
                    error = Some(
                        "Remote outcome and cost are unknown. This request will not be sent again."
                            .into(),
                    );
                }
                annotagent_storage::ConversationCallStatus::Failed => {
                    error =
                        Some("Feedback stopped before sending; no proposal was produced.".into());
                }
            }
        }
        Ok(
            serde_json::json!({"authorization":authorization,"receipt":receipt,"decision":decision,"cancelled":cancelled,"error":error}),
        )
    }
}
