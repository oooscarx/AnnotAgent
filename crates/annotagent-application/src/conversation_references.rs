//! Resolve a frozen selection against saved evidence, not the current canvas or active Project.
use crate::LocalApplication;
use annotagent_storage::{ConversationMessageInput, ConversationSelectionRef};
use anyhow::{Context, Result, bail};
use uuid::Uuid;

impl LocalApplication {
    /// Existing feedback surface for a formal annotation. Free-form text is saved,
    /// but geometry remains an explicit CAS edit; no model guesses coordinates.
    pub fn formal_conversation_feedback(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        message: Uuid,
    ) -> Result<Option<serde_json::Value>> {
        let owner = self.conversation_project_identity(project)?;
        let Some(saved) = self
            .store
            .conversation_message(&owner, conversation, message)?
        else {
            return Ok(None);
        };
        let Some(ConversationSelectionRef::FormalAnnotation {
            task_id,
            source_run_id,
            annotation_id,
            annotation_revision_id,
            expected_snapshot_sha256,
            ..
        }) = &saved.input.reference
        else {
            return Ok(None);
        };
        if *task_id != task {
            bail!("Formal feedback belongs to another Task");
        }
        self.validate_conversation_message_selection(project, conversation, &saved.input)?;
        let image = saved
            .input
            .image
            .as_ref()
            .context("Formal feedback image is missing")?;
        let image_id: annotagent_core::ImageId = image.image_id.parse()?;
        let snapshot = self.store.delivery_image_snapshot(
            &owner,
            conversation,
            task,
            image_id,
            Some(*source_run_id),
        )?;
        let annotation = snapshot
            .annotations
            .into_iter()
            .find(|annotation| annotation.id == *annotation_id)
            .context("Formal feedback annotation is missing")?;
        Ok(Some(serde_json::json!({
            "kind":"formal_annotation_feedback","status":"awaiting_explicit_edit",
            "message":saved,"subject":{
                "image_id":image_id,"image_sha256":image.sha256,"source_run_id":source_run_id,
                "annotation":annotation,"annotation_revision_id":annotation_revision_id,
                "snapshot_sha256":expected_snapshot_sha256
            },
            "model_call_supported":false,
            "reason":"Formal geometry and label edits require explicit structured values; free-form text is context only.",
            "available_actions":[{
                "id":"edit_formal_annotation","method":"POST",
                "url":format!("/api/projects/{project}/conversations/{conversation}/tasks/{task}/delivery-images/{image_id}/objects"),
                "requires_confirmation":true
            }]
        })))
    }

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
            if let Some(ConversationSelectionRef::FormalAnnotation {
                task_id,
                project_schema_revision,
                intent_revision,
                intent_sha256,
                processing_operation_id,
                batch_id,
                source_run_id,
                annotation_id,
                annotation_revision_id,
                expected_snapshot_sha256,
            }) = &input.reference
            {
                let image = input
                    .image
                    .as_ref()
                    .context("Formal annotation requires its image and content hash")?;
                let task = self
                    .conversation_tasks(project, conversation)?
                    .into_iter()
                    .find(|record| record.input.id == *task_id)
                    .context("Formal annotation belongs to another task")?;
                if task.input.schema_revision != *project_schema_revision {
                    bail!("Formal annotation Task Schema revision changed");
                }
                let delivery = self
                    .require_delivery_intake(project, conversation, *task_id)?
                    .context("Formal annotation delivery intent is missing")?;
                if delivery.revision != *intent_revision
                    || delivery.content_sha256 != *intent_sha256
                {
                    bail!("Formal annotation delivery scope changed");
                }
                let selected = delivery
                    .intent
                    .dataset_scope
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .find(|selected| {
                        selected.image_id.to_string() == image.image_id
                            && selected.content_sha256 == image.sha256
                    })
                    .context("Formal annotation image is outside the saved delivery scope")?;
                let formal = self.task_delivery_formal_result(project, conversation, *task_id)?;
                if formal["processing_operation_id"] != processing_operation_id.to_string()
                    || formal["batch_id"] != batch_id.to_string()
                {
                    bail!("Formal annotation processing source changed");
                }
                let mapped = formal["images"]
                    .as_array()
                    .and_then(|items| {
                        items.iter().find(|item| {
                            item["image_id"] == selected.image_id.to_string()
                                && item["child_run_id"] == source_run_id.to_string()
                        })
                    })
                    .context("Formal annotation child Run is not the Task result")?;
                if mapped["run_status"].is_null() {
                    bail!("Formal annotation Run has no terminal result");
                }
                let snapshot = self.store.delivery_image_snapshot(
                    &delivery.intent.project_id,
                    conversation,
                    *task_id,
                    selected.image_id,
                    Some(*source_run_id),
                )?;
                if snapshot.sha256 != *expected_snapshot_sha256
                    || !snapshot
                        .annotations
                        .iter()
                        .any(|item| item.id == *annotation_id)
                {
                    bail!("Formal annotation snapshot changed");
                }
                let revisions = self.store.list_revisions(*annotation_id)?;
                if revisions.last().map(|revision| revision.revision_id)
                    != Some(*annotation_revision_id)
                {
                    bail!("Formal annotation revision changed");
                }
                return Ok(());
            }
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
