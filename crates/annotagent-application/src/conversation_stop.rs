//! Persisted cancellation selection over existing task services. No inference on reads.
use crate::LocalApplication;
use annotagent_core::{BatchId, BatchStatus};
use annotagent_storage::{
    ConversationCallStatus, ConversationMessageInput, ConversationStopRequest,
    ConversationStopStatus, ConversationStopTargetKind, ConversationStopTargetRef,
};
use anyhow::{Context, Result};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct ConversationStopObservation {
    pub state: &'static str,
    pub description: &'static str,
}

#[cfg(test)]
#[path = "conversation_stop_tests.rs"]
mod tests;

impl LocalApplication {
    pub fn begin_conversation_stop(
        &self,
        project: &str,
        conversation: Uuid,
        input: &ConversationMessageInput,
    ) -> Result<ConversationStopRequest> {
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .begin_conversation_stop(&owner, project, conversation, input)?)
    }

    pub fn select_conversation_stop(
        &self,
        project: &str,
        conversation: Uuid,
        message: Uuid,
        target: &ConversationStopTargetRef,
    ) -> Result<ConversationStopRequest> {
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .select_conversation_stop(&owner, project, conversation, message, target)?)
    }

    pub fn conversation_stop_request(
        &self,
        project: &str,
        conversation: Uuid,
        message: Uuid,
    ) -> Result<Option<ConversationStopRequest>> {
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .conversation_stop_request(&owner, conversation, message)?)
    }

    /// Signal only cancellations already saved by the selection transaction. Replaying this
    /// never discovers a new target or creates a cancellation for a shared Journey sibling.
    pub fn signal_conversation_stop(
        &self,
        project: &str,
        conversation: Uuid,
        message: Uuid,
    ) -> Result<Vec<String>> {
        let record = self
            .conversation_stop_request(project, conversation, message)?
            .context("Stop command was not found")?;
        if record.status != ConversationStopStatus::CancelRequested {
            return Ok(Vec::new());
        }
        let target = record
            .selected_target
            .context("Stop command has no selected target")?;
        let owner = self.conversation_project_identity(project)?;
        let mut call_ids = Vec::new();
        let mut samples = Vec::new();
        match target.kind {
            ConversationStopTargetKind::Call
            | ConversationStopTargetKind::Builder
            | ConversationStopTargetKind::Authorization => {
                call_ids.push(Uuid::parse_str(&target.id)?);
                // An authorization can be cancelled in the interval before Sample admission.
                samples.push(target.id);
            }
            ConversationStopTargetKind::Sample => samples.push(target.id),
            ConversationStopTargetKind::Journey => {
                let journey = self
                    .conversation_journey_consent(
                        project,
                        conversation,
                        target.task_id,
                        target.id.parse()?,
                    )?
                    .context("Stopped Journey was not found")?;
                call_ids.push(journey.consent.builder_operation_id);
                if let Some(schema) = journey.consent.schema_proposal {
                    call_ids.push(schema.call_id);
                }
                samples.push(journey.consent.sample_operation_id.to_string());
            }
            ConversationStopTargetKind::Processing => {
                let batch_id: BatchId = target.id.parse()?;
                let batch = match self.store.get_batch(batch_id) {
                    Ok(batch) => Some(batch),
                    Err(annotagent_storage::StorageError::BatchNotFound(_)) => None,
                    Err(error) => return Err(error.into()),
                };
                if let Some(batch) = batch {
                    anyhow::ensure!(
                        batch.project_id == project,
                        "Stopped Batch belongs to another Project"
                    );
                    if batch.status == BatchStatus::Cancelled {
                        let children = self
                            .store
                            .list_batch_images(batch_id)?
                            .into_iter()
                            .filter_map(|image| image.child_run_id)
                            .collect::<std::collections::BTreeSet<_>>();
                        let active = self.active.lock().map_err(|_| {
                            anyhow::anyhow!("Run cancellation registry unavailable")
                        })?;
                        for (_, managed) in active
                            .iter()
                            .filter(|(id, managed)| children.contains(id) && managed.is_active())
                        {
                            managed.control.cancel()?;
                        }
                    }
                }
            }
        }
        let saved = self
            .store
            .conversation_call_cancellations(&owner, target.task_id)?;
        let cancellations = self
            .conversation_cancellations
            .lock()
            .map_err(|_| anyhow::anyhow!("Cancellation registry unavailable"))?;
        for call in call_ids {
            if saved.iter().any(|item| item.call_id == call) {
                if let Some(token) = cancellations.get(&call) {
                    token.cancel();
                }
            }
        }
        // Server owns Sample tokens. Only return exact, already-cancelled child IDs.
        let mut cancelled_samples = Vec::new();
        for id in samples {
            if self.store.sample_operation(&id)?.is_some_and(|sample| {
                sample.project_id == project
                    && sample.request["conversation"]["task_id"] == target.task_id.to_string()
                    && sample.request["conversation"]["conversation_id"] == conversation.to_string()
                    && matches!(sample.status.as_str(), "cancelling" | "cancelled")
            }) {
                cancelled_samples.push(id);
            }
        }
        Ok(cancelled_samples)
    }

    pub fn conversation_stop_observation(
        &self,
        project: &str,
        conversation: Uuid,
        message: Uuid,
    ) -> Result<Option<ConversationStopObservation>> {
        let record = self
            .conversation_stop_request(project, conversation, message)?
            .context("Stop command was not found")?;
        if record.status == ConversationStopStatus::Finished {
            return Ok(Some(ConversationStopObservation {
                state: "finished",
                description: "The selected operation had already ended. No other operation was stopped.",
            }));
        }
        let Some(target) = record.selected_target else {
            return Ok(None);
        };
        let owner = self.conversation_project_identity(project)?;
        let mut pending = false;
        let mut unknown = false;
        let mut operation_ids = Vec::new();
        match target.kind {
            ConversationStopTargetKind::Call => {
                if let Some(call) =
                    self.store
                        .conversation_call(&owner, target.task_id, target.id.parse()?)?
                {
                    pending = call.status == ConversationCallStatus::Reserved;
                    unknown = call.status == ConversationCallStatus::InDoubt;
                }
            }
            ConversationStopTargetKind::Builder => {
                operation_ids.push(target.id.clone());
                if let Some(builder) = self.store.conversation_builder_operation(
                    &owner,
                    target.task_id,
                    target.id.parse()?,
                )? {
                    pending = builder.status == "reserved";
                }
            }
            ConversationStopTargetKind::Sample => {
                operation_ids.push(target.id.clone());
                if let Some(sample) = self.store.sample_operation(&target.id)? {
                    pending = matches!(sample.status.as_str(), "queued" | "running" | "cancelling");
                }
            }
            ConversationStopTargetKind::Processing => {
                match self.store.get_batch(target.id.parse()?) {
                    Ok(batch) => {
                        pending = matches!(
                            batch.status,
                            BatchStatus::Pending | BatchStatus::Running | BatchStatus::Paused
                        );
                        let children = self
                            .store
                            .list_batch_images(batch.id)?
                            .into_iter()
                            .filter_map(|image| image.child_run_id)
                            .collect::<std::collections::BTreeSet<_>>();
                        let active = self.active.lock().map_err(|_| {
                            anyhow::anyhow!("Run cancellation registry unavailable")
                        })?;
                        pending |= active
                            .iter()
                            .any(|(id, managed)| children.contains(id) && managed.is_active());
                    }
                    Err(annotagent_storage::StorageError::BatchNotFound(_)) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            ConversationStopTargetKind::Journey => {
                let state = self.conversation_journey_execution_status(
                    project,
                    conversation,
                    target.task_id,
                    target.id.parse()?,
                )?;
                let journey: annotagent_storage::ConversationJourneyRecord =
                    serde_json::from_value(state["record"].clone())?;
                operation_ids.push(journey.consent.builder_operation_id.to_string());
                operation_ids.push(journey.consent.sample_operation_id.to_string());
                let schema_cancelled = if let Some(schema) = &journey.consent.schema_proposal {
                    self.store
                        .conversation_call_cancellations(&owner, target.task_id)?
                        .iter()
                        .any(|cancel| cancel.call_id == schema.call_id)
                } else {
                    false
                };
                if schema_cancelled {
                    if let Some(schema) = &journey.consent.schema_proposal {
                        operation_ids.push(schema.call_id.to_string());
                    }
                }
                // A shared Schema call serving another Journey was not cancelled by this
                // selection, so it must not make this stop appear to wait on that sibling.
                pending = (schema_cancelled && state["schema"]["status"] == "reserved")
                    || state["builder"]["status"] == "reserved"
                    || matches!(
                        state["sample"]["status"].as_str(),
                        Some("queued" | "running" | "cancelling")
                    );
                unknown = schema_cancelled && state["schema"]["status"] == "in_doubt";
            }
            ConversationStopTargetKind::Authorization => {
                operation_ids.push(target.id.clone());
                // The grant can acquire a typed operation after the snapshot but before
                // selection. Follow that same ID only, never the task's newer allowance.
                if let Some(builder) = self.store.conversation_builder_operation(
                    &owner,
                    target.task_id,
                    target.id.parse()?,
                )? {
                    pending = builder.status == "reserved";
                }
                if let Some(sample) = self.store.sample_operation(&target.id)? {
                    pending |=
                        matches!(sample.status.as_str(), "queued" | "running" | "cancelling");
                }
                if let Some(call) =
                    self.store
                        .conversation_call(&owner, target.task_id, target.id.parse()?)?
                {
                    pending |= call.status == ConversationCallStatus::Reserved;
                    unknown |= call.status == ConversationCallStatus::InDoubt;
                }
            }
        }
        for operation in operation_ids {
            let calls = self.store.conversation_operation_call_state(
                &owner,
                conversation,
                target.task_id,
                &operation,
            )?;
            pending |= calls.reserved > 0;
            unknown |= calls.in_doubt > 0;
        }
        Ok(Some(if pending {
            ConversationStopObservation {
                state: "cancel_pending",
                description: "Cancellation is saved; the selected worker is still settling. Previously sent requests may still incur charges.",
            }
        } else if unknown {
            ConversationStopObservation {
                state: "unknown",
                description: "Local execution ended. The remote completion and cost are unknown; no automatic retry was started.",
            }
        } else {
            ConversationStopObservation {
                state: "cancelled",
                description: "The selected operation is no longer active. Saved results and prior usage remain; this does not imply a refund.",
            }
        }))
    }
}
