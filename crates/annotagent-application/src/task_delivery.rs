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
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveTaskDeliveryIntent {
    pub command_id: Uuid,
    pub expected_revision: u32,
    /// Legacy same-Project identities. New upload flows should send
    /// `task_images` so the upload-observed content hash is part of the command.
    pub image_ids: Option<Vec<ImageId>>,
    #[serde(default)]
    pub task_images: Option<Vec<annotagent_storage::ConversationImageRef>>,
    pub label_spec: Option<Vec<DeliveryLabel>>,
    pub training_target: Option<TrainingTarget>,
    pub split_policy: DeliverySplitPolicy,
    /// Explicit source metadata, not inferred from filenames or label folders.
    #[serde(default)]
    pub image_metadata: std::collections::BTreeMap<ImageId, DeliveryImageMetadata>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryImageMetadata {
    pub existing_split: Option<annotagent_core::dataset_delivery::DatasetSplit>,
    pub group_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareDeliverySchema {
    pub command_id: Uuid,
    pub expected_revision: u32,
    pub expected_sha256: String,
}

/// Bounded semantic context only: no image paths, credentials or image bytes.
/// This describes the user's saved target, not permission to execute that target.
pub(crate) fn delivery_schema_goal(
    saved: Option<&TaskDeliveryRevision>,
    goal: &str,
) -> Result<String> {
    let Some(saved) = saved else {
        return Ok(goal.to_owned());
    };
    Ok(serde_json::to_string(&serde_json::json!({
        "contract":"task-delivery-semantics-v1",
        "original_user_goal":goal,
        "saved_delivery":{
            "revision":saved.revision,"content_sha256":saved.content_sha256,
            "labels":saved.intent.label_spec,"training_target":saved.intent.training_target,
            "image_count":saved.intent.dataset_scope.as_ref().map(Vec::len),
            "missing_slots":saved.intent.missing_slots(),
            "completion":"Deliver a validated original-image training package after whole-image review. A Schema or Pipeline alone is not completion."
        }
    }))?)
}

pub(crate) fn frozen_delivery_schema_goal(
    saved: &TaskDeliveryRevision,
    user_goal: &str,
) -> Result<String> {
    Ok(serde_json::to_string(
        &serde_json::json!({"contract":"task-delivery-schema-v1","delivery_revision":saved.revision,"delivery_sha256":saved.content_sha256,"saved_user_goal":user_goal,"labels":saved.intent.label_spec,"target":saved.intent.training_target,"image_count":saved.intent.dataset_scope.as_ref().map(Vec::len),"review_policy":saved.intent.review_policy,"completion":"Deliver a structurally validated original-image training package after explicit whole-image review; generating a Pipeline alone is not completion."}),
    )?)
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
    /// Explicitly selectable historical model proposals, never automatically adopted.
    pub proposals: Vec<TaskDeliveryProposal>,
}

#[derive(Debug, Serialize)]
pub struct TaskDeliveryProposal {
    pub call_id: Uuid,
    pub semantics: crate::conversation_schema::DeliverySemanticsProposal,
    pub question: Option<String>,
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
    pub(crate) fn complete_delivery_from_schema_proposal(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        call: Uuid,
        decision: &crate::conversation_schema::ConversationSchemaDecision,
    ) -> Result<Option<TaskDeliveryRevision>> {
        let view = self.task_delivery_intent(project, conversation, task)?;
        let Some(saved) = view.saved else {
            return Ok(None);
        };
        if view.missing_slots.is_empty() {
            return Ok(Some(saved));
        }
        let crate::conversation_schema::ConversationSchemaDecision::Draft {
            labels, delivery, ..
        } = decision
        else {
            return Ok(Some(saved));
        };
        let proposal = delivery
            .as_ref()
            .context("Schema result omitted complete delivery semantics")?;
        let target = proposal
            .training_target
            .clone()
            .context("Schema result omitted the requested training target")?;
        ensure!(
            proposal.labels.len() == labels.len(),
            "Schema and delivery label counts disagree"
        );
        let proposed_labels = proposal
            .labels
            .iter()
            .zip(labels)
            .map(|(proposal, stable_id)| {
                ensure!(
                    proposal
                        .existing_id
                        .as_ref()
                        .is_none_or(|existing| existing == stable_id),
                    "Schema and delivery label identities disagree"
                );
                Ok(DeliveryLabel {
                    stable_id: stable_id.clone(),
                    display_name: proposal.display_name.clone(),
                    aliases: proposal.aliases.clone(),
                    include: proposal.include.clone(),
                    exclude: proposal.exclude.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if let Some(existing) = &saved.intent.label_spec {
            ensure!(
                existing == &proposed_labels,
                "Saved delivery labels changed before Schema materialization"
            );
        }
        if let Some(existing) = &saved.intent.training_target {
            ensure!(
                existing == &target,
                "Saved training target changed before Schema materialization"
            );
        }
        let image_ids = saved
            .intent
            .dataset_scope
            .as_ref()
            .context("Task upload scope is missing")?
            .iter()
            .map(|image| image.image_id)
            .collect();
        self.save_task_delivery_intent(
            project,
            conversation,
            task,
            SaveTaskDeliveryIntent {
                command_id: call,
                expected_revision: saved.revision,
                image_ids: Some(image_ids),
                task_images: None,
                label_spec: Some(saved.intent.label_spec.clone().unwrap_or(proposed_labels)),
                training_target: Some(saved.intent.training_target.clone().unwrap_or(target)),
                split_policy: saved.intent.split_policy,
                image_metadata: std::collections::BTreeMap::new(),
            },
        )?;
        Ok(self
            .task_delivery_intent(project, conversation, task)?
            .saved)
    }

    /// Current formal result is an exact Task processing operation and its Batch child Runs.
    /// Older project Runs and processing done for another delivery revision are excluded.
    pub fn task_delivery_formal_result(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<serde_json::Value> {
        let saved = self
            .require_delivery_intake(project, conversation, task)?
            .context("Save complete delivery information before processing")?;
        let scope = saved.intent.dataset_scope.as_deref().unwrap_or_default();
        for operation in self.conversation_processing_history(project, conversation, task)? {
            if operation["authorization"]["delivery_scope"]["intent_revision"] != saved.revision
                || operation["authorization"]["delivery_scope"]["intent_sha256"]
                    != saved.content_sha256
            {
                continue;
            }
            let Some(batch_id) = operation["batch_id"].as_str() else {
                continue;
            };
            let batch_id: annotagent_core::BatchId = batch_id.parse()?;
            let batch = self.store.get_batch(batch_id)?;
            ensure!(
                batch.project_id == project,
                "Processing Batch ownership changed"
            );
            let authorized = operation["authorization"]["images"]
                .as_array()
                .context("Processing image scope missing")?;
            if authorized.len() != scope.len()
                || scope.iter().any(|image| {
                    !authorized.iter().any(|item| {
                        item["image_id"] == image.image_id.to_string()
                            && item["content_hash"] == image.content_sha256
                    })
                })
            {
                bail!("Processing image scope differs from the current delivery scope");
            }
            let images = self
                .store
                .list_batch_images_summary(batch_id)?
                .into_iter()
                .map(|item| {
                    serde_json::json!({
                        "image_id":item.image.image_id,
                        "child_run_id":item.image.child_run_id,
                        "status":item.image.status,
                        "run_status":item.run_status,
                        "error":item.image.error.or(item.terminal_reason),
                        "annotation_count":item.annotation_count,
                        "review_count":item.review_count
                    })
                })
                .collect::<Vec<_>>();
            return Ok(serde_json::json!({
                "project_id":project,"task_id":task,
                "intent_revision":saved.revision,"intent_sha256":saved.content_sha256,
                "processing_operation_id":operation["id"],"batch_id":batch.id,
                "workflow_id":operation["workflow_id"],"workflow_version":operation["version"],
                "status":batch.status,"images":images
            }));
        }
        Ok(serde_json::Value::Null)
    }

    pub fn task_delivery_review_items(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        cursor: usize,
        limit: usize,
    ) -> Result<serde_json::Value> {
        ensure!((1..=100).contains(&limit), "Review limit must be 1..100");
        let saved = self
            .require_delivery_intake(project, conversation, task)?
            .context("Save complete delivery information before reviewing")?;
        let project_schema_revision = self
            .conversation_tasks(project, conversation)?
            .into_iter()
            .find(|record| record.input.id == task)
            .context("Delivery Task not found")?
            .input
            .schema_revision;
        let scope = saved.intent.dataset_scope.as_deref().unwrap_or_default();
        ensure!(
            cursor <= scope.len(),
            "Review cursor is outside the current scope"
        );
        let formal = self.task_delivery_formal_result(project, conversation, task)?;
        let formal_images = formal["images"].as_array().cloned().unwrap_or_default();
        let project_images = self.list_project_image_summaries(project)?;
        let end = cursor.saturating_add(limit).min(scope.len());
        let mut items = Vec::new();
        for image in &scope[cursor..end] {
            let execution = formal_images
                .iter()
                .find(|item| item["image_id"] == image.image_id.to_string());
            let source_run_id = execution
                .and_then(|item| item["child_run_id"].as_str())
                .map(str::parse)
                .transpose()?;
            let state = self.task_delivery_image(
                project,
                conversation,
                task,
                image.image_id,
                source_run_id,
            )?;
            let annotations = state
                .snapshot
                .annotations
                .iter()
                .map(|annotation| {
                    let latest = self.store.list_revisions(annotation.id)?.last().cloned();
                    let preset_source_artifact_id = self.store.demo_preset_annotation_origin(
                        &saved.intent.project_id,
                        task,
                        annotation.id,
                    )?;
                    let reference = match (
                        latest.as_ref(),
                        source_run_id,
                        formal["processing_operation_id"].as_str(),
                        formal["batch_id"].as_str(),
                    ) {
                        (Some(revision), Some(run), Some(operation), Some(batch)) => Some(
                            serde_json::json!({
                                "scope":"formal_annotation","task_id":task,
                                "project_schema_revision":project_schema_revision,
                                "intent_revision":saved.revision,"intent_sha256":saved.content_sha256,
                                "processing_operation_id":operation,"batch_id":batch,"source_run_id":run,
                                "annotation_id":annotation.id,"annotation_revision_id":revision.revision_id,
                                "expected_snapshot_sha256":state.snapshot.sha256
                            }),
                        ),
                        _ => None,
                    };
                    Ok(serde_json::json!({
                        "annotation_id":annotation.id,"label":annotation.label,"value":annotation.value,
                        "review_status":annotation.review_status,
                        "origin":if annotation.source==annotagent_core::AnnotationSource::Imported{"preset_candidate"}else if annotation.source==annotagent_core::AnnotationSource::Human{"human_revision"}else{"live_model_prediction"},
                        "source_artifact_id":preset_source_artifact_id,
                        "annotation_revision_id":latest.map(|revision|revision.revision_id),
                        "source_artifact_ids":annotation.provenance.artifact_ids,
                        "feedback_available":reference.is_some(),"conversation_reference":reference
                    }))
                })
                .collect::<Result<Vec<_>>>()?;
            items.push(serde_json::json!({
                "image_id":image.image_id,"content_sha256":image.content_sha256,
                "name":project_images.iter().find(|item|item.image_id==image.image_id).map(|item|item.name.clone()),
                "url":format!("/api/projects/{project}/images/{}/content",image.image_id),
                "thumbnail_url":format!("/api/projects/{project}/images/{}/thumbnail",image.image_id),
                "processing_operation_id":formal.get("processing_operation_id"),
                "batch_id":formal.get("batch_id"),"child_run_id":source_run_id,
                "execution_status":execution.and_then(|item|item.get("status")),
                "execution_error":execution.and_then(|item|item.get("error")),
                "review_revision":state.review.as_ref().map_or(0, |review|review.revision),
                "review_decision":state.review.as_ref().map(|review|review.input.decision),
                "confirmation_current":state.confirmation_current,
                "accepted_objects":state.accepted_objects,"unresolved_objects":state.unresolved_objects,
                "snapshot_sha256":state.snapshot.sha256,"annotations":annotations
            }));
        }
        let reviews =
            self.store
                .delivery_image_reviews(&saved.intent.project_id, conversation, task)?;
        let counts = scope.iter().fold(
            std::collections::BTreeMap::from([
                ("positive", 0usize),
                ("negative", 0usize),
                ("excluded", 0usize),
            ]),
            |mut counts, image| {
                if let Some(review) = reviews.iter().find(|r| r.input.image_id == image.image_id) {
                    let key = match review.input.decision {
                        annotagent_storage::DeliveryImageDecision::PositiveComplete => "positive",
                        annotagent_storage::DeliveryImageDecision::NegativeConfirmed => "negative",
                        annotagent_storage::DeliveryImageDecision::Excluded => "excluded",
                    };
                    *counts.get_mut(key).unwrap() += 1;
                }
                counts
            },
        );
        Ok(serde_json::json!({
            "project_id":project,"task_id":task,"intent_revision":saved.revision,
            "intent_sha256":saved.content_sha256,
            "summary":{"selected":scope.len(),"positive":counts["positive"],"negative":counts["negative"],"excluded":counts["excluded"],"unreviewed":scope.len().saturating_sub(reviews.len())},
            "items":items,"next_cursor":(end<scope.len()).then_some(end.to_string())
        }))
    }

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
            delivery: None,
            kind: crate::conversation_schema::ConversationOutputKind::BoundingBox,
            labels: labels.iter().map(|l| l.stable_id.clone()).collect(),
            multi_label: false,
            attributes: std::collections::BTreeMap::new(),
            boundary_rules: rules.clone(),
            rationale:
                "Explicit user delivery labels and detection task; no inference or publication."
                    .into(),
        };
        let goal = frozen_delivery_schema_goal(&saved, &message.input.text)?;
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

    /// Adds a human missing-object revision with snapshot CAS; does not accept the image.
    pub fn create_task_delivery_object(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        image: ImageId,
        input: &annotagent_storage::DeliveryObjectCreate,
    ) -> Result<annotagent_core::AnnotationRevision> {
        let saved = self
            .require_delivery_intake(project, conversation, task)?
            .context("Save delivery information before adding objects")?;
        Ok(self.store.create_delivery_object(
            &saved.intent.project_id,
            conversation,
            task,
            image,
            input,
        )?)
    }

    /// Revises an existing formal object with snapshot CAS; does not confirm the whole image.
    pub fn edit_task_delivery_object(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        image: ImageId,
        input: &annotagent_storage::DeliveryObjectEdit,
    ) -> Result<annotagent_core::AnnotationRevision> {
        let saved = self
            .require_delivery_intake(project, conversation, task)?
            .context("Save delivery information before editing formal objects")?;
        self.store
            .edit_delivery_object(&saved.intent.project_id, conversation, task, image, input)
            .map_err(Into::into)
    }

    /// Reviews a repository-imported Demo candidate through the same delivery snapshot CAS.
    /// This records a human revision and never creates or invokes a model Run.
    pub fn review_task_demo_preset_object(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        image: ImageId,
        input: &annotagent_storage::DemoPresetObjectReviewInput,
    ) -> Result<annotagent_core::AnnotationRevision> {
        let saved = self
            .require_delivery_intake(project, conversation, task)?
            .context("Save delivery information before reviewing preset candidates")?;
        Ok(self.store.review_demo_preset_object(
            &saved.intent.project_id,
            conversation,
            task,
            image,
            input,
        )?)
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
        let mut proposals = Vec::new();
        for receipt in self
            .store
            .conversation_call_history(&owner, task)?
            .into_iter()
            .rev()
        {
            if receipt.status != annotagent_storage::ConversationCallStatus::Completed {
                continue;
            }
            let Some(evidence) = receipt.evidence else {
                continue;
            };
            let Ok(attempt) = serde_json::from_value::<
                crate::conversation_schema::ConversationSchemaAttempt,
            >(evidence) else {
                continue;
            };
            let Ok(decision) =
                crate::conversation_schema::parse_conversation_schema_response(&attempt.response)
            else {
                continue;
            };
            let (semantics, question) = match decision {
                crate::ConversationSchemaDecision::Draft { delivery, .. } => (delivery, None),
                crate::ConversationSchemaDecision::Clarify {
                    delivery, question, ..
                } => (delivery, Some(question)),
            };
            if let Some(semantics) = semantics {
                proposals.push(TaskDeliveryProposal {
                    call_id: receipt.id,
                    semantics,
                    question,
                });
            }
            if proposals.len() == 10 {
                break;
            }
        }
        Ok(TaskDeliveryView {
            saved,
            missing_slots,
            blockers,
            maximum_sample_images: 3,
            execution_authorized: false,
            proposals,
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
        let previous = self
            .store
            .task_delivery_intent(&owner, conversation, task)?;
        ensure!(
            input.image_ids.is_none() || input.task_images.is_none(),
            "Choose image_ids or task_images, not both"
        );
        let requested_images = input.task_images.as_ref().map(|images| {
            images
                .iter()
                .map(|image| {
                    Ok((
                        image
                            .image_id
                            .parse::<ImageId>()
                            .context("Task attachment contains an invalid image ID")?,
                        Some(image.sha256.as_str()),
                    ))
                })
                .collect::<Result<Vec<_>>>()
        });
        let requested_images = match requested_images {
            Some(images) => Some(images?),
            None => input
                .image_ids
                .as_ref()
                .map(|ids| ids.iter().copied().map(|id| (id, None)).collect()),
        };
        for (id, metadata) in &input.image_metadata {
            if !requested_images
                .as_ref()
                .is_some_and(|images| images.iter().any(|(selected, _)| selected == id))
            {
                bail!("Image metadata must belong to the explicitly selected scope");
            }
            if metadata.group_ids.len() > 32
                || metadata.group_ids.iter().any(|s| {
                    s.trim().is_empty() || s.len() > 128 || s.chars().any(char::is_control)
                })
            {
                bail!("Invalid source group metadata");
            }
        }
        let dataset_scope = if let Some(images) = requested_images {
            if images.len() > 100_000 {
                bail!("Delivery image scope is too large");
            }
            if images
                .iter()
                .map(|(id, _)| id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != images.len()
            {
                bail!("Delivery image scope contains duplicate identities");
            }
            let current = self.list_project_image_summaries(project)?;
            let mut selected = Vec::with_capacity(images.len());
            for (id, expected_hash) in images {
                let image = current
                    .iter()
                    .find(|image| {
                        image.image_id == id
                            && expected_hash.is_none_or(|hash| hash == image.content_hash)
                    })
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Selected image is missing, changed or belongs to another Project"
                        )
                    })?;
                let saved_metadata = previous
                    .as_ref()
                    .and_then(|saved| saved.intent.dataset_scope.as_ref())
                    .and_then(|images| {
                        images
                            .iter()
                            .find(|i| i.image_id == id && i.content_sha256 == image.content_hash)
                    });
                let metadata = input.image_metadata.get(&id);
                selected.push(DeliveryImage {
                    image_id: id,
                    content_sha256: image.content_hash.clone(),
                    content_revision: image.content_hash.clone(),
                    existing_split: metadata.map_or_else(
                        || saved_metadata.and_then(|m| m.existing_split),
                        |m| m.existing_split,
                    ),
                    group_ids: metadata
                        .map(|m| m.group_ids.clone())
                        .or_else(|| saved_metadata.map(|m| m.group_ids.clone()))
                        .unwrap_or_default(),
                });
            }
            Some(selected)
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
        let context: serde_json::Value =
            serde_json::from_str(&delivery_schema_goal(Some(&delivery), "YOLO").unwrap()).unwrap();
        assert_eq!(context["saved_delivery"]["image_count"], 3);
        assert_eq!(
            context["saved_delivery"]["missing_slots"],
            serde_json::json!(["label_spec", "training_target"])
        );
        assert!(context["saved_delivery"]["training_target"].is_null());
        assert!(context.to_string().find(&ids[3].to_string()).is_none());
        assert_eq!(
            delivery_schema_goal(None, "legacy goal").unwrap(),
            "legacy goal"
        );
        let mut renamed = delivery.clone();
        renamed.revision = 2;
        renamed.intent.label_spec = Some(vec![DeliveryLabel {
            stable_id: "stable-cup".into(),
            display_name: "水杯".into(),
            aliases: vec!["cup".into()],
            include: "完整或遮挡的杯子".into(),
            exclude: "图案".into(),
        }]);
        let updated: serde_json::Value = serde_json::from_str(
            &delivery_schema_goal(Some(&renamed), "original cup request").unwrap(),
        )
        .unwrap();
        assert_eq!(
            updated["saved_delivery"]["labels"][0]["stable_id"],
            "stable-cup"
        );
        assert_eq!(updated["saved_delivery"]["labels"][0]["exclude"], "图案");
        assert_eq!(updated["saved_delivery"]["revision"], 2);
    }

    #[test]
    fn uploaded_task_images_and_complete_schema_proposal_form_one_frozen_intake() {
        let temp = tempfile::tempdir().unwrap();
        let app = LocalApplication::new(temp.path()).unwrap();
        let project = "TEST-upload-intake";
        app.create_project(project,"version: 1\nproject:\n  name: TEST upload intake\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n").unwrap();
        let source = temp.path().join("TEST-upload-source");
        std::fs::create_dir(&source).unwrap();
        let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/demo-packs/object-detection-review/1.0.0/images");
        for index in 1..=6 {
            std::fs::copy(
                pack.join(format!("desk_{index:02}.png")),
                source.join(format!("desk_{index:02}.png")),
            )
            .unwrap();
        }
        let upload = app.import_images_with_report(project, &source).unwrap();
        assert_eq!(upload.images.len(), 6);
        let task_images = upload
            .images
            .iter()
            .map(|image| annotagent_storage::ConversationImageRef {
                image_id: image.image_id.to_string(),
                sha256: image.content_hash.clone(),
            })
            .collect::<Vec<_>>();
        let conversation = app.create_project_conversation(project).unwrap();
        let command: annotagent_storage::ConversationSendInput = serde_json::from_value(
            serde_json::json!({
                "message":{"id":Uuid::new_v4(),"text":"框出杯子并交付 YOLO Detection 数据集","image":null},
                "task_images":task_images,
                "task_id":null,
                "schema_revision":app.project_goal(project).unwrap()["revision"],
                "mode":"execute"
            }),
        )
        .unwrap();
        let receipt = app
            .send_project_conversation_message(project, conversation, &command)
            .unwrap();
        let partial = app
            .task_delivery_intent(project, conversation, receipt.task_id)
            .unwrap();
        assert_eq!(partial.saved.as_ref().unwrap().revision, 1);
        assert_eq!(
            partial
                .saved
                .as_ref()
                .unwrap()
                .intent
                .dataset_scope
                .as_ref()
                .unwrap()
                .len(),
            6
        );
        assert_eq!(
            partial.missing_slots,
            vec![DeliverySlot::LabelSpec, DeliverySlot::TrainingTarget]
        );
        let read = app
            .mainline_task_read_model(project, conversation, receipt.task_id)
            .unwrap();
        assert_eq!(read["available_actions"].as_array().unwrap().len(), 1);
        assert_eq!(
            read["available_actions"][0]["id"],
            "build_and_test_pipeline"
        );
        assert_eq!(
            read["available_actions"][0]["scope"]["images"]
                .as_array()
                .unwrap()
                .len(),
            6
        );
        assert_eq!(
            read["available_actions"][0]["scope"]["maximum_sample_images"],
            3
        );
        let owner = app.conversation_project_identity(project).unwrap();
        let unresolved_call = Uuid::new_v4();
        let unresolved_scope = "a".repeat(64);
        app.store()
            .authorize_conversation_calls(
                &owner,
                &annotagent_storage::ConversationCallGrant {
                    id: unresolved_call,
                    task_id: receipt.task_id,
                    scope_hash: unresolved_scope.clone(),
                    maximum_calls: 2,
                    expires_at: chrono::Utc::now() + chrono::Duration::minutes(10),
                },
            )
            .unwrap();
        app.store()
            .reserve_conversation_call(
                &owner,
                receipt.task_id,
                unresolved_call,
                &unresolved_scope,
                &"b".repeat(64),
            )
            .unwrap();
        app.store()
            .finish_conversation_call(
                &owner,
                receipt.task_id,
                unresolved_call,
                annotagent_storage::ConversationCallStatus::InDoubt,
                serde_json::json!({"failure":{
                    "stage":"provider_request",
                    "category":"interrupted",
                    "http_status":504
                }}),
            )
            .unwrap();
        let unresolved = app
            .mainline_task_read_model(project, conversation, receipt.task_id)
            .unwrap();
        assert!(
            unresolved["available_actions"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            unresolved["result_diagnostics"][0]["code"],
            "provider_outcome_unknown"
        );
        assert!(
            unresolved["blockers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|blocker| { blocker["code"] == "schema_attempt_requires_resolution" })
        );
        assert_eq!(
            app.send_project_conversation_message(project, conversation, &command)
                .unwrap(),
            receipt
        );

        let call = Uuid::new_v4();
        let decision = crate::conversation_schema::ConversationSchemaDecision::Draft {
            kind: crate::conversation_schema::ConversationOutputKind::BoundingBox,
            labels: vec!["cup".into()],
            multi_label: false,
            attributes: std::collections::BTreeMap::new(),
            boundary_rules: vec!["Exclude printed cup pictures".into()],
            rationale: "Explicit detection and export request".into(),
            delivery: Some(crate::conversation_schema::DeliverySemanticsProposal {
                labels: vec![crate::conversation_schema::DeliveryLabelProposal {
                    existing_id: None,
                    display_name: "杯子".into(),
                    aliases: vec!["cup".into()],
                    include: "真实杯子".into(),
                    exclude: "杯子图案".into(),
                }],
                training_target: Some(annotagent_core::dataset_delivery::TrainingTarget {
                    annotation_kind: annotagent_core::TaskKind::BoundingBox,
                    framework: "ultralytics".into(),
                    export_profile: "ultralytics_yolo_detection".into(),
                    profile_revision: 1,
                }),
            }),
        };
        let complete = app
            .complete_delivery_from_schema_proposal(
                project,
                conversation,
                receipt.task_id,
                call,
                &decision,
            )
            .unwrap()
            .unwrap();
        assert_eq!(complete.revision, 2);
        assert_eq!(
            complete.intent.label_spec.as_ref().unwrap()[0].stable_id,
            "cup"
        );
        assert_eq!(
            complete.intent.dataset_scope,
            partial.saved.unwrap().intent.dataset_scope
        );
        assert_eq!(
            app.complete_delivery_from_schema_proposal(
                project,
                conversation,
                receipt.task_id,
                call,
                &decision,
            )
            .unwrap()
            .unwrap(),
            complete
        );
    }
}
