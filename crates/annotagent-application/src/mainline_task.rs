//! Passive Task delivery projection and bounded, server-authorized local advancement.
use crate::{LocalApplication, PrepareDeliverySchema, require_delivery_schema};
use annotagent_storage::{
    ConversationCallStatus, ConversationJourneyRecord, DeliveryPackagePhase, SampleOperation,
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

fn journey_sample_operation(journeys: &[Value]) -> Option<&Value> {
    journeys
        .iter()
        .find_map(|journey| journey.get("sample").filter(|sample| !sample.is_null()))
}

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
    pub(crate) fn latest_processing_candidate(
        &self,
        operations: &[SampleOperation],
    ) -> Result<Option<Value>> {
        for operation in operations {
            if operation.status != "succeeded" {
                continue;
            }
            let Some(sample) = self.store.get_workflow_sample_test_by_id(&operation.id)? else {
                continue;
            };
            let current_draft = self.store.get_workflow_draft(&sample.draft_id).ok();
            if sample.project_id == operation.project_id
                && sample.draft_id == operation.draft_id
                && sample.status.allows_publication()
                && current_draft.as_ref().is_some_and(|draft| {
                    draft.project_id == sample.project_id
                        && draft.revision == sample.draft_revision
                        && draft.content_hash == sample.draft_content_hash
                })
            {
                return Ok(Some(json!({
                    "draft_id":sample.draft_id,"draft_revision":sample.draft_revision,
                    "draft_content_hash":sample.draft_content_hash,
                    "sample_test_id":sample.id,"sample_status":sample.status
                })));
            }
        }
        Ok(None)
    }

    pub(crate) fn pending_journey_sample_action(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        journeys: &[Value],
    ) -> Result<Option<Value>> {
        for journey in journeys {
            if journey
                .get("sample")
                .is_some_and(|sample| !sample.is_null())
            {
                continue;
            }
            let Some(builder) = journey.get("builder") else {
                continue;
            };
            let Some(evidence) = builder.get("evidence") else {
                continue;
            };
            if builder["status"] != "completed"
                || evidence["outcome"] != "draft_ready_for_human_review"
            {
                continue;
            }
            let record: ConversationJourneyRecord =
                serde_json::from_value(journey["record"].clone())?;
            let consent = record.effective_consent();
            let draft_id = evidence["draft_id"].as_str().unwrap_or_default();
            let exact_draft = self.store.get_workflow_draft(draft_id).is_ok_and(|draft| {
                draft.project_id == project
                    && evidence["draft_revision"].as_u64() == Some(draft.revision)
                    && evidence["draft_content_hash"].as_str() == Some(draft.content_hash.as_str())
            });
            let active = !record.revoked
                && consent.expires_at > chrono::Utc::now()
                && exact_draft
                && self
                    .validate_conversation_journey_data(project, conversation, consent)
                    .is_ok();
            let root = format!(
                "/api/projects/{project}/conversations/{conversation}/tasks/{task}/journey-consents/{}",
                record.consent.id
            );
            return Ok(Some(json!({
                "id":"test_pipeline_samples",
                "state":if active{"requires_confirmation"}else{"blocked"},
                "method":"GET","url":root.clone(),
                "execution_method":"POST","execution_url":format!("{root}/execution"),
                "requires_confirmation":true,
                "reason":if active{"exact_saved_journey_sample_requires_confirmation"}else{"saved_journey_sample_scope_stale"},
                "scope":{
                    "journey_consent_id":record.consent.id,
                    "sample_operation_id":consent.sample_operation_id,
                    "draft_id":draft_id,"draft_revision":evidence["draft_revision"],
                    "draft_content_hash":evidence["draft_content_hash"],
                    "images":consent.images,"allowed_models":consent.allowed_models,
                    "maximum_sample_calls":consent.maximum_sample_calls,
                    "expires_at":consent.expires_at
                }
            })));
        }
        Ok(None)
    }

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
        let messages = calls
            .iter()
            .enumerate()
            .filter_map(|(index, call)| {
                let created_at = call.started_at.as_ref()?;
                let status = serde_json::to_value(call.status).ok()?;
                let text = match call.status {
                    ConversationCallStatus::Reserved => "Model request is in progress.",
                    ConversationCallStatus::Completed => "Model request completed.",
                    ConversationCallStatus::Failed => {
                        "Model request failed; inspect the safe receipt stage and category."
                    }
                    ConversationCallStatus::InDoubt => {
                        "Model request outcome is unknown; it will not be retried automatically."
                    }
                };
                Some(json!({
                    "id":call.id,"task_id":task,"sequence":index + 1,
                    "kind":"system_receipt","text":text,"created_at":created_at,
                    "source":{"kind":"model_call","id":call.id},
                    "receipt":{"status":status,"stage":call.stage,"completed_at":call.completed_at,
                        "duration_ms":call.duration_ms,"failure":call.failure}
                }))
            })
            .collect::<Vec<_>>();
        let processing = self.conversation_processing_history(project, conversation, task)?;
        let journeys = self.conversation_journey_history(project, conversation, task)?;
        let sample_operations =
            self.store
                .conversation_sample_operations(project, conversation, task)?;
        let processing_candidate = self.latest_processing_candidate(&sample_operations)?;
        let pending_journey_sample =
            self.pending_journey_sample_action(project, conversation, task, &journeys)?;
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
        let active_operations =
            self.store
                .agent_ui_active_operations(&owner, project, conversation, task)?;
        let mut actions = Vec::new();
        let mut blockers = delivery
            .blockers
            .iter()
            .map(|message| json!({"code":"delivery_intent_blocked","message":message}))
            .collect::<Vec<_>>();
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
            blockers.push(json!({
                "code":"delivery_intake_incomplete",
                "message":"Complete the Task delivery intake before planning or execution."
            }));
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
        } else if processing.is_empty() && processing_candidate.is_some() {
            let candidate = processing_candidate.as_ref().unwrap();
            let draft_id = candidate["draft_id"].as_str().unwrap();
            let sample_test_id = candidate["sample_test_id"].as_str().unwrap();
            actions.push(json!({
                "id":"start_delivery_processing","state":"requires_confirmation","method":"GET",
                "url":format!("/api/projects/{project}/processing-preview?draft_id={draft_id}&sample_test_id={sample_test_id}"),
                "requires_confirmation":true,
                "reason":"exact_delivery_processing_scope_requires_confirmation",
                "scope":{
                    "delivery_revision":delivery.saved.as_ref().map(|saved|saved.revision),
                    "delivery_sha256":delivery.saved.as_ref().map(|saved|saved.content_sha256.clone()),
                    "images":delivery.saved.as_ref().and_then(|saved|saved.intent.dataset_scope.clone()).unwrap_or_default(),
                    "draft":candidate,
                    "preview_freezes_model_bindings_destination_and_cost":true
                }
            }));
        } else if processing.is_empty()
            && let Some(pending_journey_sample) = pending_journey_sample
        {
            actions.push(pending_journey_sample);
        } else if processing.is_empty() && journey_sample_operation(&journeys).is_some() {
            let sample = journey_sample_operation(&journeys).expect("checked Journey sample");
            let status = sample["status"].as_str().unwrap_or("unknown");
            actions.push(json!({
                "id":"inspect_pipeline_samples",
                "state":if matches!(status,"queued"|"running"|"cancelling"|"succeeded"){"available"}else{"blocked"},
                "method":"GET",
                "url":format!("/api/projects/{project}/sample-operations/{}",sample["id"].as_str().unwrap_or_default()),
                "requires_confirmation":false,
                "reason":if matches!(status,"failed"|"interrupted"|"cancelled"){"sample_terminal_without_success_no_automatic_retry"}else{"sample_operation_already_exists"},
                "scope":{"sample_operation_id":sample["id"],"status":status}
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
            "messages":messages,
            "steps":[
                {"id":"schema","kind":"schema","title":"Task Schema","state":if schema.is_some(){"completed"}else{"waiting"},"status":if schema.is_some(){"completed"}else if delivery.saved.is_none()||!delivery.missing_slots.is_empty(){"blocked"}else{"ready"},"request_completed":schema.is_some(),"task_completed":false},
                {"id":"processing","kind":"processing","title":"Dataset processing","state":if processing_completed{"completed"}else if processing.is_empty(){"waiting"}else{"running"},"status":if processing_completed{"completed"}else if processing.is_empty(){"awaiting_approval"}else{"running"},"request_completed":processing_completed,"task_completed":false},
                {"id":"whole_image_review","kind":"whole_image_review","title":"Whole-image review","state":if selected_images>0&&pending_reviews==0{"completed"}else{"waiting"},"status":if selected_images>0&&pending_reviews==0{"completed"}else if processing_completed{"ready"}else{"blocked"},"request_completed":selected_images>0&&pending_reviews==0,"task_completed":false},
                {"id":"training_package","kind":"training_package","title":"Training package","state":package_state,"status":if package_ready{"completed"}else if package_state=="failed"{"failed"}else if package_state=="running"{"running"}else if selected_images>0&&pending_reviews==0{"awaiting_approval"}else{"blocked"},"request_completed":package_ready,"task_completed":package_ready}
            ],
            "package":{
                "consents":consents,
                "jobs":packages.iter().map(|job| json!({
                    "id":job.id,"phase":job.phase,"intent_revision":job.snapshot.delivery.revision,
                    "snapshot_sha256":job.snapshot_sha256,"error":job.error
                })).collect::<Vec<_>>()
            },
            "available_actions":actions,
            "active_operation_ids":active_operations.iter().map(|operation| operation.id.clone()).collect::<Vec<_>>(),
            "blockers":blockers,
            "completion":{
                "model_request_completed":model_request_completed,
                "processing_completed":processing_completed,
                "package_ready":package_ready,
                "task_completed":package_ready,
                "status":if package_ready{"package_ready"}else{"incomplete"}
            },
            "links":{
                "self":format!("{root}/workspace"),
                "thread":format!("{root}/thread"),
                "visual_selections":format!("{root}/visual-selections"),
                "capability_readiness":format!("{root}/capability-readiness"),
                "review_work_items":format!("{root}/delivery-review-items"),
                "package_consents":format!("{root}/delivery-package-consents"),
                "advance":format!("{root}/advance")
            }
        });
        let revision = annotagent_image_tools::sha256(&serde_json::to_vec(&projection)?);
        projection["read_model_revision"] = json!(revision);
        projection["revision"] = json!(revision);

        // G0 domain aliases are derived entirely from the authoritative fields above.
        // They let the UI consume one server-owned Task view without reconstructing
        // identity, action availability or routes from unrelated responses.
        let saved_intent = delivery.saved.as_ref().map(|saved| &saved.intent);
        projection["intake"] = json!({
            "missing_slots":delivery.missing_slots.iter().map(|slot| match slot {
                annotagent_core::dataset_delivery::DeliverySlot::DatasetScope => "dataset_scope",
                annotagent_core::dataset_delivery::DeliverySlot::LabelSpec => "label_rules",
                annotagent_core::dataset_delivery::DeliverySlot::TrainingTarget => "training_target",
            }).collect::<Vec<_>>(),
            "dataset_scope":saved_intent.and_then(|intent|intent.dataset_scope.clone()),
            "label_rules":saved_intent.and_then(|intent|intent.label_spec.clone()),
            "training_target":saved_intent.and_then(|intent|intent.training_target.clone())
        });
        projection["message_projection"] = json!({
            "url":format!("{root}/thread"),
            "embedded_kinds":["system_receipt"],
            "note":"The Task view embeds safe model-call receipts. The Thread endpoint contains persisted user messages only; no assistant reply is fabricated."
        });
        projection["actions"] = Value::Array(
            projection["available_actions"]
                .as_array()
                .expect("mainline actions are an array")
                .iter()
                .map(|action| {
                    let state = action["state"].as_str().unwrap_or("blocked");
                    json!({
                        "id":action["id"],
                        "kind":action["id"],
                        "available":state != "blocked",
                        "reason":action["reason"].as_str().unwrap_or_default(),
                        "requires_approval":action["requires_confirmation"],
                        "scope_revision":revision,
                        "method":action["method"],
                        "url":action["url"],
                        "execution_method":action.get("execution_method").cloned().unwrap_or(Value::Null),
                        "execution_url":action.get("execution_url").cloned().unwrap_or(Value::Null),
                        "scope":action.get("scope").cloned().unwrap_or(Value::Null)
                    })
                })
                .collect(),
        );
        if formal_result.is_object() {
            projection["review_work_item_id"] = json!(task);
        }
        if let Some(job) = packages.first() {
            projection["package_id"] = json!(job.id);
        }
        if let Some(job) = packages
            .iter()
            .find(|job| job.phase == DeliveryPackagePhase::Ready)
        {
            projection["completion"]["package_id"] = json!(job.id);
            projection["completion"]["download_url"] =
                json!(format!("{root}/delivery-packages/{}/download", job.id));
        }
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

#[cfg(test)]
mod tests {
    use super::journey_sample_operation;
    use serde_json::json;

    #[test]
    fn journey_sample_operation_never_treats_a_missing_sample_as_terminal() {
        let journeys = vec![
            json!({"sample":null}),
            json!({"sample":{"id":"saved-operation","status":"running"}}),
        ];
        assert_eq!(
            journey_sample_operation(&journeys).unwrap()["id"],
            "saved-operation"
        );
        assert!(journey_sample_operation(&[json!({"sample":null})]).is_none());
    }
}
