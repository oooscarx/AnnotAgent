//! Frozen message-to-result interpretation. Model output only proposes human work;
//! it never writes feedback, marks `HumanVerified`, changes a Schema or starts repair.
use crate::{ConversationSchemaExecution, LocalApplication};
use annotagent_core::{FinalCandidateProjection, ImageId, ModelResponse, VisionModelProvider};
use annotagent_storage::{ConversationCallReceipt, ConversationMessage, ConversationSelectionRef};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConversationFeedbackContext {
    pub message: ConversationMessage,
    pub candidate: FinalCandidateProjection,
    pub expected_feedback_sequence: u64,
    pub sample_content_hash: String,
    pub pixels_supplied: bool,
}

impl ConversationFeedbackContext {
    pub fn digest(&self) -> Result<String> {
        Ok(annotagent_image_tools::sha256(&serde_json::to_vec(self)?))
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ConversationFeedbackResult {
    pub receipt: ConversationCallReceipt,
    pub decision: Result<crate::conversation_feedback_intent::ConversationFeedbackDecision, String>,
}

impl LocalApplication {
    /// Read only. Candidate identity comes exclusively from the persisted message.
    pub fn conversation_feedback_context(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        message: Uuid,
    ) -> Result<ConversationFeedbackContext> {
        let owner = self.conversation_project_identity(project)?;
        let saved = self
            .store
            .conversation_message(&owner, conversation, message)?
            .context("Saved feedback message not found")?;
        let Some(ConversationSelectionRef::SampleCandidate {
            task_id,
            sample_test_id,
            candidate_id,
            source_artifact_id,
            ..
        }) = &saved.input.reference
        else {
            bail!("Feedback needs an explicit saved candidate reference; choose its scope first");
        };
        if *task_id != task {
            bail!("Feedback message belongs to another task");
        }
        if saved.input.text.len() > 4000 {
            bail!("Feedback is too long for a bounded interpretation; shorten it to 4000 bytes");
        }
        self.validate_conversation_message_selection(project, conversation, &saved.input)?;
        let image = saved
            .input
            .image
            .as_ref()
            .context("Feedback image reference is missing")?;
        let current =
            self.project_image_path(project, ImageId(Uuid::parse_str(&image.image_id)?))?;
        if annotagent_image_tools::sha256(&std::fs::read(current)?) != image.sha256 {
            bail!("Referenced image bytes changed; no feedback interpretation was started");
        }
        let sample = self
            .store
            .get_workflow_sample_test_by_id(sample_test_id)?
            .context("Saved sample missing")?;
        let index = sample
            .inputs
            .iter()
            .position(|item| item.image_id == image.image_id)
            .context("Saved sample image missing")?;
        let projection = &sample.report.samples[index].projection;
        let candidate = projection
            .final_candidates
            .iter()
            .chain(
                projection
                    .review_candidates
                    .iter()
                    .map(|review| &review.candidate),
            )
            .find(|candidate| {
                candidate.outcome.id == *candidate_id
                    && candidate.source_artifact_id.to_string() == source_artifact_id.to_string()
            })
            .context("Terminal candidate missing")?
            .clone();
        let feedback = self
            .store
            .sample_feedback(sample_test_id, &image.image_id)?;
        Ok(ConversationFeedbackContext {
            message: saved,
            candidate,
            expected_feedback_sequence: feedback.last().map_or(0, |item| item.sequence),
            sample_content_hash: sample.draft_content_hash,
            pixels_supplied: false,
        })
    }

    pub fn read_conversation_feedback(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        call: Uuid,
    ) -> Result<Option<ConversationFeedbackResult>> {
        let Some(receipt) = self.conversation_call_receipt(project, conversation, task, call)?
        else {
            return Ok(None);
        };
        let evidence = receipt
            .evidence
            .as_ref()
            .context("Feedback call is pending or has unknown evidence")?;
        if evidence["phase"] != "feedback_text" {
            bail!("This receipt is not a feedback interpretation");
        }
        let context: ConversationFeedbackContext =
            serde_json::from_value(evidence["context"]["subject"].clone())?;
        let owner = self.conversation_project_identity(project)?;
        let cancelled = evidence["cancelled"] == true
            || self
                .store
                .conversation_call_cancellations(&owner, task)?
                .iter()
                .any(|item| item.call_id == call);
        let decision = if cancelled {
            Err("Feedback interpretation was cancelled; no proposal may be applied".into())
        } else if receipt.status == annotagent_storage::ConversationCallStatus::Completed {
            let response: ModelResponse = serde_json::from_value(evidence["response"].clone())?;
            crate::conversation_feedback_intent::parse_conversation_feedback_response(
                &response,
                &serde_json::to_value(&context)?,
            )
            .map_err(|error| error.to_string())
        } else {
            Err("Feedback outcome and cost are unknown; do not reissue this call".into())
        };
        Ok(Some(ConversationFeedbackResult { receipt, decision }))
    }

    /// Explicit command: create/reuse only the existing human correction request.
    /// The model never submits the answer, marks verification or launches repair.
    pub fn prepare_conversation_feedback_request(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        call: Uuid,
    ) -> Result<Option<annotagent_storage::ConversationHumanRequest>> {
        use annotagent_storage::{ConversationHumanRequestInput, ConversationHumanRequestStatus};
        let saved = self
            .read_conversation_feedback(project, conversation, task, call)?
            .context("Feedback interpretation is not saved")?;
        let decision = saved.decision.map_err(|error| anyhow::anyhow!(error))?;
        let crate::ConversationFeedbackDecision::RequestCorrection {
            reason, question, ..
        } = decision
        else {
            return Ok(None);
        };
        let context: ConversationFeedbackContext = serde_json::from_value(
            saved
                .receipt
                .evidence
                .as_ref()
                .context("Feedback evidence missing")?["context"]["subject"]
                .clone(),
        )?;
        let Some(ConversationSelectionRef::SampleCandidate {
            sample_test_id,
            candidate_id,
            ..
        }) = &context.message.input.reference
        else {
            bail!("Saved feedback lacks its subject reference");
        };
        let image = context
            .message
            .input
            .image
            .as_ref()
            .context("Saved feedback lacks its image reference")?;
        let id = Uuid::new_v5(&call, b"feedback-human-request-v1");
        let reason = match reason {
            crate::ConversationFeedbackReason::PoorBoundary => "poor_boundary",
            crate::ConversationFeedbackReason::WrongLabel => "wrong_label",
            crate::ConversationFeedbackReason::WrongTarget => "wrong_target",
        };
        let input = ConversationHumanRequestInput {
            id,
            task_id: task,
            conversation_id: conversation,
            sample_test_id: sample_test_id.clone(),
            image_id: image.image_id.clone(),
            content_hash: image.sha256.clone(),
            outcome_id: candidate_id.clone(),
            expected_feedback_sequence: context.expected_feedback_sequence,
            reason_code: reason.into(),
            question,
            resume_checkpoint_ref: Uuid::new_v5(&id, b"prepared-repair-draft"),
        };
        let requests = self.conversation_human_requests(project, conversation, task)?;
        // Restore a receipt, not a new write, after subsequent human edits.
        if let Some(existing) = requests.iter().find(|item| item.input.id == id) {
            if existing.input != input {
                bail!("Feedback human request idempotency conflict");
            }
            return Ok(Some(existing.clone()));
        }
        let current = self.conversation_feedback_context(
            project,
            conversation,
            task,
            context.message.input.id,
        )?;
        if current != context {
            bail!(
                "Feedback subject changed after interpretation. Saved proposal remains historical; no human request was changed"
            );
        }
        // Human request IDs currently address outcomes, not Artifact pairs. Reject
        // ambiguous duplicate outcome IDs rather than applying to another lineage.
        let sample = self
            .store
            .get_workflow_sample_test_by_id(sample_test_id)?
            .context("Saved sample missing")?;
        let index = sample
            .inputs
            .iter()
            .position(|item| item.image_id == image.image_id)
            .context("Saved image missing")?;
        let projection = &sample.report.samples[index].projection;
        if projection
            .final_candidates
            .iter()
            .chain(
                projection
                    .review_candidates
                    .iter()
                    .map(|item| &item.candidate),
            )
            .filter(|item| item.outcome.id == *candidate_id)
            .count()
            != 1
        {
            bail!("Human correction requires an unambiguous terminal outcome ID");
        }
        if let Some(existing) = requests.iter().find(|item| {
            item.input.sample_test_id == *sample_test_id
                && item.input.image_id == image.image_id
                && item.status == ConversationHumanRequestStatus::Pending
        }) {
            if existing.input.outcome_id == *candidate_id
                && existing.input.expected_feedback_sequence == context.expected_feedback_sequence
                && !existing.deferred
            {
                return Ok(Some(existing.clone()));
            }
            bail!(
                "An existing human request on this image must be resolved first; it was not cancelled or answered"
            );
        }
        self.validate_conversation_correction_subject(project, &input)?;
        let owner = self.conversation_project_identity(project)?;
        Ok(Some(
            self.store.create_conversation_feedback_human_request(
                &owner,
                &input,
                &saved.receipt,
            )?,
        ))
    }

    /// Caller resolves and explicitly authorizes the model binding and exact context digest.
    /// Existing receipt recovery occurs before live subject validation, but never mutates data.
    pub async fn execute_conversation_feedback(
        &self,
        project: &str,
        execution: &ConversationSchemaExecution,
        context: &ConversationFeedbackContext,
        provider: &dyn VisionModelProvider,
        cancellation: CancellationToken,
    ) -> Result<ConversationFeedbackResult> {
        if execution.call_id.is_nil() {
            bail!("Feedback call identity cannot be empty");
        }
        let frozen = json!({"contract":"conversation-feedback-v1","subject":context,"remote_model":execution.remote_model,"scope_hash":execution.scope_hash});
        if let Some(receipt) = self.conversation_call_receipt(
            project,
            execution.conversation_id,
            execution.task_id,
            execution.call_id,
        )? {
            if receipt
                .evidence
                .as_ref()
                .is_none_or(|evidence| evidence["context"] != frozen)
            {
                bail!(
                    "Feedback call is pending, indeterminate or conflicts with this frozen context; no new call was sent"
                );
            }
            return self
                .read_conversation_feedback(
                    project,
                    execution.conversation_id,
                    execution.task_id,
                    execution.call_id,
                )?
                .context("Feedback receipt missing");
        }
        let current = self.conversation_feedback_context(
            project,
            execution.conversation_id,
            execution.task_id,
            context.message.input.id,
        )?;
        if &current != context {
            bail!("Feedback subject or feedback revision changed before inference");
        }
        let scoped = self
            .conversation_text_provider(
                project,
                execution.conversation_id,
                execution.task_id,
                &execution.scope_hash,
                &execution.remote_model,
                provider,
            )?
            .for_feedback_call(execution.call_id, frozen.clone());
        let attempt = crate::conversation_feedback_intent::propose_conversation_feedback(
            &scoped,
            &execution.remote_model,
            &context.message.input.text,
            &serde_json::to_value(context)?,
            cancellation.clone(),
        )
        .await;
        if cancellation.is_cancelled() {
            self.cancel_conversation_schema(
                project,
                execution.conversation_id,
                execution.task_id,
                execution.call_id,
            )?;
        }
        if let Some(saved) = self.read_conversation_feedback(
            project,
            execution.conversation_id,
            execution.task_id,
            execution.call_id,
        )? {
            if saved
                .receipt
                .evidence
                .as_ref()
                .is_none_or(|evidence| evidence["context"] != frozen)
            {
                bail!("Concurrent feedback request conflicts with this frozen context");
            }
            Ok(saved)
        } else {
            attempt?;
            bail!("Feedback was not admitted; no interpretation receipt exists")
        }
    }
}

#[cfg(test)]
#[path = "conversation_feedback_tests.rs"]
mod tests;
