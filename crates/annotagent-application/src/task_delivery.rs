//! Server-owned intake. Saving intent never authorizes a model or starts a job.
use crate::LocalApplication;
use annotagent_core::{
    ImageId,
    dataset_delivery::{
        DeliveryImage, DeliveryLabel, DeliveryReviewPolicy, DeliverySlot, DeliverySplitPolicy,
        TaskDeliveryIntent, TrainingTarget,
    },
};
use annotagent_storage::TaskDeliveryRevision;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveTaskDeliveryIntent {
    pub command_id: Uuid,
    pub expected_revision: u32,
    pub image_ids: Option<Vec<ImageId>>,
    pub label_spec: Option<Vec<DeliveryLabel>>,
    pub training_target: Option<TrainingTarget>,
    pub split_policy: DeliverySplitPolicy,
}

#[derive(Debug, Serialize)]
pub struct TaskDeliveryView {
    pub saved: Option<TaskDeliveryRevision>,
    pub missing_slots: Vec<DeliverySlot>,
    pub blockers: Vec<String>,
    pub maximum_sample_images: usize,
    /// This projection cannot grant permission to execute or certify a package.
    pub execution_authorized: bool,
}

impl LocalApplication {
    pub fn task_delivery_intent(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<TaskDeliveryView> {
        let owner = self.conversation_project_identity(project)?;
        let saved = self
            .store
            .task_delivery_intent(&owner, conversation, task)?;
        let missing_slots = saved.as_ref().map_or_else(
            || {
                vec![
                    DeliverySlot::DatasetScope,
                    DeliverySlot::LabelSpec,
                    DeliverySlot::TrainingTarget,
                ]
            },
            |value| value.intent.missing_slots(),
        );
        let mut blockers = Vec::new();
        if let Some(value) = &saved {
            if let Some(target) = &value.intent.training_target {
                if !target.is_detection_preset() {
                    blockers.push("This training target has no complete delivery preset; its annotation kind was preserved.".into());
                }
            }
            if let Some(images) = &value.intent.dataset_scope {
                let current = self.list_project_image_summaries(project)?;
                for image in images {
                    if !current.iter().any(|item| {
                        item.image_id == image.image_id && item.content_hash == image.content_sha256
                    }) {
                        blockers.push("Selected images changed or were removed; confirm a new delivery scope.".into());
                        break;
                    }
                }
            }
        }
        Ok(TaskDeliveryView {
            saved,
            missing_slots,
            blockers,
            maximum_sample_images: 3,
            execution_authorized: false,
        })
    }

    pub fn save_task_delivery_intent(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        input: SaveTaskDeliveryIntent,
    ) -> Result<TaskDeliveryView> {
        let owner = self.conversation_project_identity(project)?;
        // Check task ownership even on an empty or malicious image scope.
        self.store
            .task_delivery_intent(&owner, conversation, task)?;
        let dataset_scope = if let Some(ids) = input.image_ids {
            if ids.len() > 100_000 {
                bail!("Delivery image scope is too large");
            }
            let current = self.list_project_image_summaries(project)?;
            let mut images = Vec::with_capacity(ids.len());
            for id in ids {
                let image = current
                    .iter()
                    .find(|image| image.image_id == id)
                    .ok_or_else(|| {
                        anyhow::anyhow!("Selected image does not belong to this Project")
                    })?;
                images.push(DeliveryImage {
                    image_id: id,
                    content_sha256: image.content_hash.clone(),
                    content_revision: image.content_hash.clone(),
                    existing_split: None,
                    group_ids: vec![],
                });
            }
            Some(images)
        } else {
            None
        };
        let intent = TaskDeliveryIntent {
            version: 1,
            project_id: owner,
            conversation_id: conversation,
            task_id: task,
            dataset_scope,
            label_spec: input.label_spec,
            training_target: input.training_target,
            split_policy: input.split_policy,
            review_policy: DeliveryReviewPolicy::HumanWholeImage,
        };
        self.store
            .save_task_delivery_intent(input.command_id, input.expected_revision, &intent)?;
        self.task_delivery_intent(project, conversation, task)
    }
}
