//! Resolve a frozen selection against saved evidence, not the current canvas or active Project.
use crate::LocalApplication;
use annotagent_storage::{ConversationMessageInput, ConversationSelectionRef};
use anyhow::{Context, Result, bail};
use uuid::Uuid;

impl LocalApplication {
    pub(crate) fn validate_conversation_message_selection(
        &self,
        project: &str,
        conversation: Uuid,
        input: &ConversationMessageInput,
    ) -> Result<()> {
        let Some(ConversationSelectionRef::SampleCandidate {
            task_id,
            project_schema_revision,
            draft_id,
            draft_revision,
            sample_test_id,
            candidate_id,
            source_artifact_id,
        }) = &input.reference
        else {
            return Ok(());
        };
        let image = input
            .image
            .as_ref()
            .context("Selected candidate requires its image and content hash")?;
        let task = self
            .conversation_tasks(project, conversation)?
            .into_iter()
            .find(|task| task.input.id == *task_id)
            .context("Selected candidate belongs to another task")?;
        if &task.input.schema_revision != project_schema_revision {
            bail!("Selected task Schema revision changed");
        }
        let operation = self
            .store
            .sample_operation(sample_test_id)?
            .context("Selected sample operation not found")?;
        if operation.project_id != project
            || operation.draft_id != *draft_id
            || operation.request["conversation"]["conversation_id"] != conversation.to_string()
            || operation.request["conversation"]["task_id"] != task_id.to_string()
        {
            bail!("Selected sample belongs to another conversation task or Draft");
        }
        let sample = self
            .store
            .get_workflow_sample_test_by_id(sample_test_id)?
            .context("Selected sample report is not saved")?;
        if sample.project_id != project
            || sample.draft_id != *draft_id
            || sample.draft_revision != *draft_revision
            || !sample.report.sandbox
            || sample.inputs.len() != sample.report.samples.len()
        {
            bail!("Selected sample revision does not match saved Sandbox evidence");
        }
        let index = sample
            .inputs
            .iter()
            .position(|item| item.image_id == image.image_id && item.content_hash == image.sha256)
            .context("Selected candidate image does not match the saved sample")?;
        let projection = &sample.report.samples[index].projection;
        let matches = projection
            .final_candidates
            .iter()
            .chain(
                projection
                    .review_candidates
                    .iter()
                    .map(|item| &item.candidate),
            )
            .filter(|item| {
                item.outcome.id == *candidate_id
                    && item.source_artifact_id.to_string() == source_artifact_id.to_string()
            })
            .count();
        if matches != 1 {
            bail!(
                "Selected object must identify exactly one terminal sample candidate and Artifact"
            );
        }
        // Storage checks the current image hash and task owner again in its append transaction.
        // This message is context only: no feedback, formal annotation, label or authority changes.
        Ok(())
    }
}
