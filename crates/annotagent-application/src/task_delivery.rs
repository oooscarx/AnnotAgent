//! Server-owned intake. Saving intent never authorizes a model or starts a job.
use crate::LocalApplication;
use annotagent_core::{
    ImageId, ReviewStatus, RunId,
    dataset_delivery::{
        DeliveryImage, DeliveryLabel, DeliveryReviewPolicy, DeliverySlot, DeliverySplitPolicy,
        TaskDeliveryIntent, TrainingTarget,
    },
};
use annotagent_storage::{
    DeliveryImageReview, DeliveryImageReviewInput, DeliveryImageSnapshot, TaskDeliveryRevision,
};
use anyhow::{Context, Result, bail};
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareDeliverySchema {
    pub command_id: Uuid,
    pub expected_revision: u32,
    pub expected_sha256: String,
}

pub fn require_delivery_schema(
    saved: Option<&TaskDeliveryRevision>,
    definition: &annotagent_storage::ConversationSchemaDefinition,
) -> Result<()> {
    let Some(saved) = saved else {
        return Ok(());
    };
    let goal: serde_json::Value = serde_json::from_str(&definition.goal)
        .context("Prepare the Schema from the saved delivery goals before building a Pipeline")?;
    let expected_labels = saved
        .intent
        .label_spec
        .as_ref()
        .context("Delivery labels missing")?
        .iter()
        .map(|l| l.stable_id.clone())
        .collect::<Vec<_>>();
    if definition.task.kind != annotagent_core::TaskKind::BoundingBox
        || definition.task.labels != expected_labels
        || goal["contract"] != "task-delivery-schema-v1"
        || goal["delivery_revision"] != saved.revision
        || goal["delivery_sha256"] != saved.content_sha256
        || goal["labels"] != serde_json::to_value(&saved.intent.label_spec)?
        || goal["target"] != serde_json::to_value(&saved.intent.training_target)?
    {
        bail!(
            "Schema does not match the current delivery revision. Prepare a new Schema from the saved goals; no model request was sent."
        );
    }
    Ok(())
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

#[derive(Debug, Serialize)]
pub struct TaskDeliveryImageView {
    pub sources: Vec<annotagent_storage::DeliveryRunSource>,
    pub intent_revision: u32,
    pub intent_sha256: String,
    pub snapshot: DeliveryImageSnapshot,
    pub review: Option<DeliveryImageReview>,
    /// Not package readiness: class, split and lineage validation still apply.
    pub confirmation_current: bool,
    pub accepted_objects: usize,
    pub unresolved_objects: usize,
    pub notice: String,
}

pub(crate) fn selected_delivery_image(
    delivery: Option<&TaskDeliveryRevision>,
    image: ImageId,
) -> bool {
    delivery.is_none_or(|value| {
        value
            .intent
            .dataset_scope
            .as_ref()
            .is_some_and(|scope| scope.iter().any(|selected| selected.image_id == image))
    })
}

impl LocalApplication {
    /// User already supplied exact semantics; reuse the existing human Schema draft service,
    /// without a redundant planning model call or any Project/Workflow publication.
    pub fn prepare_delivery_schema(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        input: &PrepareDeliverySchema,
    ) -> Result<annotagent_storage::ConversationSchemaDraft> {
        let saved = self
            .require_delivery_intake(project, conversation, task)?
            .context("Save delivery information first")?;
        if saved.revision != input.expected_revision
            || saved.content_sha256 != input.expected_sha256
        {
            bail!("Delivery goals changed; reload before preparing the Schema");
        }
        let record = self
            .conversation_tasks(project, conversation)?
            .into_iter()
            .find(|t| t.input.id == task)
            .context("Owned delivery Task not found")?;
        let message = self
            .store
            .conversation_message(
                &saved.intent.project_id,
                conversation,
                record.input.source_message_id,
            )?
            .context("Task goal message missing")?;
        let labels = saved.intent.label_spec.as_ref().context("Labels missing")?;
        let rules = labels
            .iter()
            .map(|label| {
                serde_json::to_string(label)
                    .map(|json| format!("Label semantics (untrusted task data): {json}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let decision = crate::conversation_schema::ConversationSchemaDecision::Draft {
            kind: crate::conversation_schema::ConversationOutputKind::BoundingBox,
            labels: labels.iter().map(|l| l.stable_id.clone()).collect(),
            multi_label: false,
            attributes: std::collections::BTreeMap::new(),
            boundary_rules: rules.clone(),
            rationale:
                "Explicit user delivery labels and detection task; no inference or publication."
                    .into(),
        };
        let goal = serde_json::to_string(
            &serde_json::json!({"contract":"task-delivery-schema-v1","delivery_revision":saved.revision,"delivery_sha256":saved.content_sha256,"saved_user_goal":message.input.text,"labels":saved.intent.label_spec,"target":saved.intent.training_target,"image_count":saved.intent.dataset_scope.as_ref().map(Vec::len),"review_policy":saved.intent.review_policy,"completion":"Deliver a structurally validated original-image training package after explicit whole-image review; generating a Pipeline alone is not completion."}),
        )?;
        self.store
            .create_human_schema_with_clarification(
                &saved.intent.project_id,
                task,
                input.command_id,
                &annotagent_storage::ConversationSchemaDefinition {
                    goal,
                    task: decision
                        .task_config(task)?
                        .context("Detection task missing")?,
                    boundary_rules: rules,
                },
                None,
            )
            .map_err(Into::into)
    }
    /// Restores the exact image/source selection. Never chooses the latest global Run.
    pub fn task_delivery_image(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        image: ImageId,
        source_run: Option<RunId>,
    ) -> Result<TaskDeliveryImageView> {
        let saved = self
            .require_delivery_intake(project, conversation, task)?
            .ok_or_else(|| anyhow::anyhow!("Save delivery information before reviewing images"))?;
        let snapshot = self.store.delivery_image_snapshot(
            &saved.intent.project_id,
            conversation,
            task,
            image,
            source_run,
        )?;
        let review = self
            .store
            .delivery_image_reviews(&saved.intent.project_id, conversation, task)?
            .into_iter()
            .find(|r| r.input.image_id == image);
        let confirmation_current = review.as_ref().is_some_and(|r| {
            r.snapshot.sha256 == snapshot.sha256 && r.input.source_run_id == source_run
        });
        let accepted_objects = snapshot
            .annotations
            .iter()
            .filter(|a| a.review_status == ReviewStatus::HumanAccepted)
            .count();
        let unresolved_objects = snapshot
            .annotations
            .iter()
            .filter(|a| {
                !matches!(
                    a.review_status,
                    ReviewStatus::HumanAccepted | ReviewStatus::Rejected
                )
            })
            .count();
        let notice = if confirmation_current {
            "Whole-image decision saved for this snapshot. Package validation is separate."
        } else if review.is_some() {
            "The saved decision does not match this image/source snapshot. Inspect and confirm again."
        } else {
            "This image has not been confirmed as a whole. Empty results and accepted objects are not whole-image completion."
        }.into();
        Ok(TaskDeliveryImageView {
            sources: self.store.delivery_run_sources(
                &saved.intent.project_id,
                conversation,
                task,
                image,
            )?,
            intent_revision: saved.revision,
            intent_sha256: saved.content_sha256,
            snapshot,
            review,
            confirmation_current,
            accepted_objects,
            unresolved_objects,
            notice,
        })
    }

    /// Saves only a scoped human receipt, never accepts objects, starts a model or packages data.
    pub fn confirm_task_delivery_image(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        input: &DeliveryImageReviewInput,
    ) -> Result<DeliveryImageReview> {
        let saved = self
            .require_delivery_intake(project, conversation, task)?
            .ok_or_else(|| anyhow::anyhow!("Save delivery information before reviewing images"))?;
        Ok(self.store.confirm_delivery_image(
            &saved.intent.project_id,
            conversation,
            task,
            input,
        )?)
    }

    /// Opt-in delivery Tasks must resolve all slots before planning admission.
    /// Existing non-delivery Tasks retain their existing workflow semantics.
    pub fn require_delivery_intake(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<Option<TaskDeliveryRevision>> {
        let view = self.task_delivery_intent(project, conversation, task)?;
        if view.saved.is_some() {
            if !view.missing_slots.is_empty() {
                bail!(
                    "Complete delivery information before planning: {:?}",
                    view.missing_slots
                );
            }
            if !view.blockers.is_empty() {
                bail!(
                    "Delivery information cannot be used: {}",
                    view.blockers.join("; ")
                );
            }
        }
        Ok(view.saved)
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn samples_are_selected_from_the_delivery_scope_not_first_project_images() {
        let ids = (0..6).map(|_| ImageId::new()).collect::<Vec<_>>();
        let intent: TaskDeliveryIntent = serde_json::from_value(serde_json::json!({
            "version":1,"project_id":Uuid::new_v4(),"conversation_id":Uuid::new_v4(),"task_id":Uuid::new_v4(),
            "dataset_scope":ids[3..].iter().map(|id| serde_json::json!({"image_id":id,"content_sha256":"a".repeat(64),"content_revision":"1","existing_split":null,"group_ids":[]})).collect::<Vec<_>>(),
            "label_spec":null,"training_target":null,"split_policy":{"train_percent":80,"seed":0,"preserve_existing":true,"keep_known_groups_together":true},"review_policy":"human_whole_image"
        })).unwrap();
        let delivery = TaskDeliveryRevision {
            revision: 1,
            content_sha256: "b".repeat(64),
            intent,
        };
        let selected = ids
            .iter()
            .copied()
            .filter(|id| selected_delivery_image(Some(&delivery), *id))
            .take(3)
            .collect::<Vec<_>>();
        assert_eq!(selected, ids[3..]);
        assert!(ids.iter().all(|id| selected_delivery_image(None, *id)));
    }
}
