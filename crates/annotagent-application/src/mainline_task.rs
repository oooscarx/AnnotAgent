//! Passive Task delivery projection and bounded, server-authorized local advancement.
use crate::{LocalApplication, PrepareDeliverySchema, require_delivery_schema};
use annotagent_storage::{ConversationCallStatus, DeliveryPackagePhase};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvanceTaskInput {
    pub command_id: Uuid,
    pub expected_read_model_revision: String,
    pub action_id: String,
}

#[derive(Debug, Serialize)]
pub struct AdvanceTaskReceipt {
    pub command_id: Uuid,
    pub action_id: String,
    pub replayed: bool,
    pub result: Value,
    pub workspace: Value,
}

impl LocalApplication {
    /// Reconciles existing durable records. It never grants permission or dispatches work.
    pub fn mainline_task_read_model(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<Value> {
        ensure!(
            self.conversation_tasks(project, conversation)?
                .iter()
                .any(|item| item.input.id == task),
            "Task belongs to another conversation"
        );
        let owner = self.conversation_project_identity(project)?;
        let delivery = self.task_delivery_intent(project, conversation, task)?;
        let schema = if delivery.saved.is_some() {
            self.human_conversation_schema_drafts(project, conversation, task)?
                .into_iter()
                .find(|draft| {
                    require_delivery_schema(delivery.saved.as_ref(), &draft.definition).is_ok()
                })
        } else {
            None
        };
        let calls = self.store.conversation_call_history(&owner, task)?;
        let model_request_completed = calls.iter().any(|call| {
            matches!(
                call.status,
                ConversationCallStatus::Completed
                    | ConversationCallStatus::Failed
                    | ConversationCallStatus::InDoubt
            )
        });
        let processing = self.conversation_processing_history(project, conversation, task)?;
        let processing_completed = processing.iter().any(|operation| {
            matches!(
                operation.get("phase").and_then(Value::as_str),
                Some("completed" | "completed_with_review" | "partial")
            ) || matches!(
                operation
                    .get("batch")
                    .and_then(|v| v.get("status"))
                    .and_then(Value::as_str),
                Some("completed" | "completed_with_review" | "partial")
            )
        });
        let consents = if delivery.saved.is_some() {
            self.training_package_consents(project, conversation, task)?
        } else {
            Vec::new()
        };
        let packages = self.store.delivery_packages(&owner, conversation, task)?;
        let package_ready = packages
            .iter()
            .any(|job| job.phase == DeliveryPackagePhase::Ready);
        let package_state = match packages.first().map(|job| job.phase) {
            Some(DeliveryPackagePhase::Ready) => "completed",
            Some(DeliveryPackagePhase::Failed | DeliveryPackagePhase::Cancelled) => "failed",
            Some(_) => "running",
            None => "waiting",
        };
        let formal_result = if delivery.saved.is_some()
            && delivery.missing_slots.is_empty()
            && delivery.blockers.is_empty()
        {
            self.task_delivery_formal_result(project, conversation, task)?
        } else {
            Value::Null
        };

        let (selected_images, reviewed_images, current_reviews, pending_reviews) =
            if let Some(saved) = &delivery.saved {
                let scope = saved.intent.dataset_scope.as_deref().unwrap_or_default();
                let reviews = self
                    .store
                    .delivery_image_reviews(&owner, conversation, task)?;
                let mut current = 0usize;
                for image in scope {
                    let review = reviews
                        .iter()
                        .find(|review| review.input.image_id == image.image_id);
                    if let Some(review) = review {
                        if self
                            .store
                            .delivery_image_snapshot(
                                &owner,
                                conversation,
                                task,
                                image.image_id,
                                review.input.source_run_id,
                            )
                            .is_ok_and(|snapshot| snapshot.sha256 == review.snapshot.sha256)
                        {
                            current += 1;
                        }
                    }
                }
                (
                    scope.len(),
                    reviews.len(),
                    current,
                    scope.len().saturating_sub(current),
                )
            } else {
                (0, 0, 0, 0)
            };

        let root = format!("/api/projects/{project}/conversations/{conversation}/tasks/{task}");
        let mut actions = Vec::new();
        let mut blockers = delivery.blockers.clone();
        if delivery.saved.is_none() || !delivery.missing_slots.is_empty() {
            actions.push(json!({
                "id":"save_delivery_intake","state":"available","method":"POST",
                "url":format!("{root}/delivery-intent"),"requires_confirmation":true,
                "reason":if delivery.saved.is_none() {"delivery_intake_missing"} else {"delivery_intake_incomplete"}
            }));
            actions.push(json!({
                "id":"propose_delivery_semantics","state":"requires_confirmation","method":"GET",
                "url":format!("{root}/schema-preview"),"requires_confirmation":true,
                "reason":"text_only_schema_call_can_propose_missing_delivery_semantics"
            }));
            blockers.push("delivery_intake_incomplete".into());
        } else if !delivery.blockers.is_empty() {
            actions.push(json!({
                "id":"prepare_delivery_schema","state":"blocked","method":"POST",
                "url":format!("{root}/advance"),"requires_confirmation":false,
                "reason":"delivery_intake_blocked"
            }));
        } else if schema.is_none() {
            actions.push(json!({
                "id":"prepare_delivery_schema","state":"authorized","method":"POST",
                "url":format!("{root}/advance"),"requires_confirmation":false,
                "reason":null
            }));
        } else if processing.is_empty() {
            actions.push(json!({
                "id":"build_and_test_pipeline","state":"requires_confirmation","method":"GET",
                "url":format!("{root}/builder-preview"),"requires_confirmation":true,
                "reason":"builder_and_image_permissions_are_separate"
            }));
        } else if pending_reviews > 0 {
            actions.push(json!({
                "id":"review_delivery_images","state":"available","method":"GET",
                "url":format!("{root}/delivery-review-items"),"requires_confirmation":false,
                "reason":"whole_image_review_pending"
            }));
        } else if !package_ready && !consents.iter().any(|consent| consent.state == "armed") {
            actions.push(json!({
                "id":"authorize_training_package","state":"requires_confirmation","method":"POST",
                "url":format!("{root}/delivery-package-consents"),"requires_confirmation":true,
                "reason":"exact_delivery_revision_required"
            }));
        }

        let mut projection = json!({
            "contract_version":"mainline-task-v1",
            "project_id":project,"project_owner_id":owner,
            "conversation_id":conversation,"task_id":task,
            "delivery":delivery,
            "schema":schema,
            "review_summary":{
                "selected_images":selected_images,"saved_review_receipts":reviewed_images,
                "current_reviews":current_reviews,"pending_reviews":pending_reviews
            },
            "formal_source":formal_result,
            "steps":[
                {"kind":"schema","state":if schema.is_some(){"completed"}else{"waiting"},"request_completed":schema.is_some(),"task_completed":false},
                {"kind":"processing","state":if processing_completed{"completed"}else if processing.is_empty(){"waiting"}else{"running"},"request_completed":processing_completed,"task_completed":false},
                {"kind":"whole_image_review","state":if selected_images>0&&pending_reviews==0{"completed"}else{"waiting"},"request_completed":selected_images>0&&pending_reviews==0,"task_completed":false},
                {"kind":"training_package","state":package_state,"request_completed":package_ready,"task_completed":package_ready}
            ],
            "package":{
                "consents":consents,
                "jobs":packages.iter().map(|job| json!({
                    "id":job.id,"phase":job.phase,"intent_revision":job.snapshot.delivery.revision,
                    "snapshot_sha256":job.snapshot_sha256,"error":job.error
                })).collect::<Vec<_>>()
            },
            "available_actions":actions,
            "blockers":blockers,
            "completion":{
                "model_request_completed":model_request_completed,
                "processing_completed":processing_completed,
                "package_ready":package_ready,
                "task_completed":package_ready
            }
        });
        let revision = annotagent_image_tools::sha256(&serde_json::to_vec(&projection)?);
        projection["read_model_revision"] = json!(revision);
        Ok(projection)
    }

    /// Executes only a local action exposed as `authorized` by the exact current projection.
    pub fn advance_mainline_task(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        input: &AdvanceTaskInput,
    ) -> Result<AdvanceTaskReceipt> {
        ensure!(
            !input.command_id.is_nil(),
            "A non-empty command ID is required"
        );

        // A lost response may be replayed after the projection changed. The existing
        // human-Schema command is the durable receipt and rejects changed definitions.
        if input.action_id == "prepare_delivery_schema" {
            if let Some(saved) = self
                .human_conversation_schema_drafts(project, conversation, task)?
                .into_iter()
                .find(|draft| draft.source_request_id == Some(input.command_id))
            {
                let delivery = self.require_delivery_intake(project, conversation, task)?;
                ensure!(
                    require_delivery_schema(delivery.as_ref(), &saved.definition).is_ok(),
                    "Saved command belongs to an older delivery scope"
                );
                return Ok(AdvanceTaskReceipt {
                    command_id: input.command_id,
                    action_id: input.action_id.clone(),
                    replayed: true,
                    result: serde_json::to_value(saved)?,
                    workspace: self.mainline_task_read_model(project, conversation, task)?,
                });
            }
        }

        let before = self.mainline_task_read_model(project, conversation, task)?;
        ensure!(
            before["read_model_revision"] == input.expected_read_model_revision,
            "Task changed; reload before advancing"
        );
        let action = before["available_actions"]
            .as_array()
            .and_then(|actions| {
                actions
                    .iter()
                    .find(|action| action["id"] == input.action_id)
            })
            .context("Action is not available for the current Task")?;
        ensure!(
            action["state"] == "authorized",
            "Action requires its existing explicit preview or confirmation"
        );
        if input.action_id != "prepare_delivery_schema" {
            bail!("Unsupported automatic Task action")
        }
        let delivery = self
            .require_delivery_intake(project, conversation, task)?
            .context("Delivery intent missing")?;
        let schema = self.prepare_delivery_schema(
            project,
            conversation,
            task,
            &PrepareDeliverySchema {
                command_id: input.command_id,
                expected_revision: delivery.revision,
                expected_sha256: delivery.content_sha256,
            },
        )?;
        Ok(AdvanceTaskReceipt {
            command_id: input.command_id,
            action_id: input.action_id.clone(),
            replayed: false,
            result: serde_json::to_value(schema)?,
            workspace: self.mainline_task_read_model(project, conversation, task)?,
        })
    }
}
