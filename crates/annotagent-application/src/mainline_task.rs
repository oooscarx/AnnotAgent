//! Passive Task delivery projection and bounded, server-authorized local advancement.
use crate::{LocalApplication, PrepareDeliverySchema, require_delivery_schema};
use annotagent_storage::{
    ConversationCallBudget, ConversationCallReceipt, ConversationCallStatus,
    ConversationHumanRequestStatus, ConversationJourneyRecord, DeliveryPackagePhase,
    SampleOperation,
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

fn call_result_diagnostic(call: &ConversationCallReceipt, calls_url: &str) -> Option<Value> {
    let (code, category, safe_action, provider_received) = match call.status {
        ConversationCallStatus::InDoubt => (
            "provider_outcome_unknown",
            "remote_outcome",
            "inspect_receipt_and_resolve_unknown",
            Value::Null,
        ),
        ConversationCallStatus::Failed
            if call.failure.as_ref().is_some_and(|failure| {
                failure.stage == annotagent_core::ModelFailureStage::PrepareRequest
            }) =>
        {
            (
                "provider_request_not_sent",
                "request_admission",
                "fix_configuration_and_authorize_new_attempt",
                json!(false),
            )
        }
        ConversationCallStatus::Failed
            if call.failure.as_ref().is_some_and(|failure| {
                failure.category == annotagent_core::ModelFailureCategory::InvalidStructuredOutput
            }) =>
        {
            (
                "model_response_invalid_structure",
                "response_validation",
                "inspect_receipt_before_new_authorization",
                json!(true),
            )
        }
        _ => return None,
    };
    Some(json!({
        "code":code,"category":category,"state":"blocked",
        "source":{"kind":"model_call","id":call.id},
        "stage":call.stage,"failure":call.failure,
        "provider_received":provider_received,
        "automatic_retry":false,"preserves_existing_results":true,
        "safe_action":{"id":safe_action,"method":"GET","url":calls_url}
    }))
}

fn sample_result_diagnostics(operation: &SampleOperation, report: &Value) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    let sample_url = format!(
        "/api/projects/{}/sample-operations/{}",
        operation.project_id, operation.id
    );
    for (index, sample) in report["samples"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let failures = sample["failure_classes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let invalid_artifact = failures.iter().any(|failure| failure == "invalid_artifact");
        let provider_or_infrastructure_failure = failures.iter().any(|failure| {
            matches!(
                failure.as_str(),
                Some("provider_failure" | "infrastructure_failure" | "budget_limit")
            )
        });
        if invalid_artifact {
            diagnostics.push(json!({
                "code":"candidate_projection_failed","category":"result_projection",
                "state":"blocked","source":{"kind":"sample_test","id":operation.id,"image_index":index},
                "failure_classes":failures,"automatic_retry":false,
                "preserves_existing_results":true,
                "safe_action":{"id":"inspect_saved_artifact","method":"GET","url":sample_url}
            }));
            continue;
        }
        let no_candidates = sample["projection"]["final_candidates"]
            .as_array()
            .is_none_or(Vec::is_empty)
            && sample["projection"]["review_candidates"]
                .as_array()
                .is_none_or(Vec::is_empty);
        if sample["empty"].as_bool().unwrap_or(false)
            && !sample["failed"].as_bool().unwrap_or(false)
            && no_candidates
            && !provider_or_infrastructure_failure
        {
            diagnostics.push(json!({
                "code":"legal_empty_detection","category":"result",
                "state":"completed","source":{"kind":"sample_test","id":operation.id,"image_index":index},
                "failure_classes":failures,"automatic_retry":false,
                "human_negative_recorded":false,"preserves_existing_results":true,
                "safe_action":{"id":"inspect_empty_result","method":"GET","url":sample_url}
            }));
        }
    }
    diagnostics
}

fn authorization_result_diagnostic(
    budget: &ConversationCallBudget,
    budget_url: &str,
) -> Option<Value> {
    let (code, reason) = if budget.revoked {
        ("authorization_revoked", "saved_authorization_was_revoked")
    } else if budget.current_grant.expires_at <= chrono::Utc::now() {
        ("authorization_expired", "saved_authorization_expired")
    } else if budget.used_calls >= budget.current_grant.maximum_calls {
        (
            "task_call_budget_exhausted",
            "saved_model_call_allowance_is_exhausted",
        )
    } else {
        return None;
    };
    Some(json!({
        "code":code,"category":"authorization","state":"blocked","reason":reason,
        "source":{"kind":"call_grant","id":budget.current_grant.id},
        "scope":{"task_id":budget.current_grant.task_id,
            "maximum_calls":budget.current_grant.maximum_calls,
            "used_calls":budget.used_calls,"expires_at":budget.current_grant.expires_at,
            "revoked":budget.revoked},
        "automatic_retry":false,"preserves_existing_results":true,
        "safe_action":{"id":"inspect_task_authorization_budget","method":"GET","url":budget_url}
    }))
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
            let record: ConversationJourneyRecord =
                serde_json::from_value(journey["record"].clone())?;
            let consent = record.effective_consent();
            let root = format!(
                "/api/projects/{project}/conversations/{conversation}/tasks/{task}/journey-consents/{}",
                record.consent.id
            );
            if matches!(
                journey["dispatch"]["status"].as_str(),
                Some("queued" | "running")
            ) {
                return Ok(Some(json!({
                    "id":"inspect_automatic_sample_progress",
                    "state":"available","method":"GET","url":format!("{root}/execution"),
                    "requires_confirmation":false,
                    "reason":"authorized_journey_is_automatically_continuing",
                    "scope":{
                        "journey_consent_id":record.consent.id,
                        "builder_operation_id":consent.builder_operation_id,
                        "sample_operation_id":consent.sample_operation_id,
                        "images":consent.images,"allowed_models":consent.allowed_models,
                        "maximum_sample_calls":consent.maximum_sample_calls,
                        "expires_at":consent.expires_at,
                        "dispatch_status":journey["dispatch"]["status"]
                    }
                })));
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
            let dispatch_error = journey
                .get("dispatch")
                .and_then(|dispatch| dispatch.get("error"))
                .and_then(Value::as_str);
            let draft_id = evidence["draft_id"].as_str().unwrap_or_default();
            let exact_draft = self.store.get_workflow_draft(draft_id).is_ok_and(|draft| {
                draft.project_id == project
                    && evidence["draft_revision"].as_u64() == Some(draft.revision)
                    && evidence["draft_content_hash"].as_str() == Some(draft.content_hash.as_str())
            });
            let active = !record.revoked
                && consent.expires_at > chrono::Utc::now()
                && exact_draft
                && dispatch_error.is_none()
                && self
                    .validate_conversation_journey_data(project, conversation, consent)
                    .is_ok();
            return Ok(Some(json!({
                "id":"test_pipeline_samples",
                "state":if active{"requires_confirmation"}else{"blocked"},
                "method":"GET","url":root.clone(),
                "execution_method":"POST","execution_url":format!("{root}/execution"),
                "requires_confirmation":true,
                "reason":if active{"exact_saved_journey_sample_requires_confirmation"}else if dispatch_error.is_some(){"saved_journey_sample_execution_failed"}else{"saved_journey_sample_scope_stale"},
                "failure":dispatch_error.map(|message|json!({"stage":"sample_admission","category":"validation","message":message})),
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
        let calls_url =
            format!("/api/projects/{project}/conversations/{conversation}/tasks/{task}/calls");
        let mut result_diagnostics = calls
            .iter()
            .filter_map(|call| call_result_diagnostic(call, &calls_url))
            .collect::<Vec<_>>();
        let budget_url =
            format!("/api/projects/{project}/conversations/{conversation}/tasks/{task}/budget");
        if let Some(budget) = self.store.conversation_call_budget(&owner, task)?
            && let Some(diagnostic) = authorization_result_diagnostic(&budget, &budget_url)
        {
            result_diagnostics.push(diagnostic);
        }
        let processing = self.conversation_processing_history(project, conversation, task)?;
        let journeys = self.conversation_journey_history(project, conversation, task)?;
        let sample_operations =
            self.store
                .conversation_sample_operations(project, conversation, task)?;
        for operation in &sample_operations {
            if let Some(sample) = self.store.get_workflow_sample_test_by_id(&operation.id)? {
                result_diagnostics.extend(sample_result_diagnostics(
                    operation,
                    &serde_json::to_value(sample.report)?,
                ));
            }
        }
        let processing_candidate = self.latest_processing_candidate(&sample_operations)?;
        let pending_sample_reviews = self
            .conversation_human_requests(project, conversation, task)?
            .into_iter()
            .filter(|request| {
                request.status == ConversationHumanRequestStatus::Pending
                    && !request.deferred
                    && processing_candidate.as_ref().is_some_and(|sample| {
                        sample["sample_test_id"] == request.input.sample_test_id
                    })
            })
            .collect::<Vec<_>>();
        let pending_journey_sample =
            self.pending_journey_sample_action(project, conversation, task, &journeys)?;
        let preset_review_ready = self.store.is_demo_preset_task(&owner, conversation, task)?;
        let processing_completed = preset_review_ready
            || processing.iter().any(|operation| {
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
        let formal_result = if preset_review_ready {
            json!({
                "kind":"preset_candidate_import",
                "status":"needs_review",
                "live_inference_occurred":false,
                "model_run_id":null
            })
        } else if delivery.saved.is_some()
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
        if delivery.saved.is_none() {
            actions.push(json!({
                "id":"save_delivery_intake","state":"available","method":"POST",
                "url":format!("{root}/delivery-intent"),"requires_confirmation":true,
                "reason":"delivery_intake_missing"
            }));
            blockers.push(json!({
                "code":"delivery_intake_incomplete",
                "message":"Complete the Task delivery intake before planning or execution."
            }));
        } else if delivery.blockers.is_empty()
            && !delivery.missing_slots.is_empty()
            && delivery.saved.as_ref().is_some_and(|saved| {
                saved
                    .intent
                    .dataset_scope
                    .as_ref()
                    .is_some_and(|images| !images.is_empty())
            })
            && delivery.missing_slots.iter().all(|slot| {
                matches!(
                    slot,
                    annotagent_core::dataset_delivery::DeliverySlot::LabelSpec
                        | annotagent_core::dataset_delivery::DeliverySlot::TrainingTarget
                )
            })
        {
            actions.push(json!({
                "id":"build_and_test_pipeline","state":"requires_confirmation","method":"GET",
                "url":format!("{root}/journey-preview"),"requires_confirmation":true,
                "reason":"confirm_one_bounded_schema_builder_sample_scope",
                "scope":{
                    "images":delivery.saved.as_ref().and_then(|saved|saved.intent.dataset_scope.clone()),
                    "maximum_sample_images":delivery.maximum_sample_images,
                    "includes":["schema_proposal","builder","sample"],
                    "missing_semantics":delivery.missing_slots
                }
            }));
        } else if !pending_sample_reviews.is_empty() {
            actions.push(json!({
                "id":"review_sample_results","state":"available","method":"GET",
                "url":format!("{root}/visual-selections"),"requires_confirmation":false,
                "reason":"sample_candidates_require_human_judgment",
                "scope":{
                    "sample_test_id":processing_candidate.as_ref().map(|sample|sample["sample_test_id"].clone()),
                    "pending_request_ids":pending_sample_reviews.iter().map(|request|request.input.id).collect::<Vec<_>>(),
                    "pending_count":pending_sample_reviews.len()
                }
            }));
        } else if !delivery.blockers.is_empty() {
            actions.push(json!({
                "id":"prepare_delivery_schema","state":"blocked","method":"POST",
                "url":format!("{root}/advance"),"requires_confirmation":false,
                "reason":"delivery_intake_blocked"
            }));
        } else if preset_review_ready {
            if pending_reviews > 0 {
                actions.push(json!({
                    "id":"review_delivery_images","state":"available","method":"GET",
                    "url":format!("{root}/delivery-review-items"),"requires_confirmation":false,
                    "reason":"preset_candidates_require_human_whole_image_review"
                }));
            } else if !package_ready && !consents.iter().any(|consent| consent.state == "armed") {
                actions.push(json!({
                    "id":"authorize_training_package","state":"requires_confirmation","method":"POST",
                    "url":format!("{root}/delivery-package-consents"),"requires_confirmation":true,
                    "reason":"exact_delivery_revision_required"
                }));
            }
        } else if schema.is_none() {
            actions.push(json!({
                "id":"prepare_delivery_schema","state":"authorized","method":"POST",
                "url":format!("{root}/advance"),"requires_confirmation":false,
                "reason":null
            }));
        } else if processing.is_empty()
            && let Some(candidate) = processing_candidate.as_ref()
        {
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
                {"id":"schema","kind":"schema","title":"Task Schema","state":if schema.is_some()||preset_review_ready{"completed"}else{"waiting"},"status":if schema.is_some()||preset_review_ready{"completed"}else if delivery.saved.is_none()||!delivery.missing_slots.is_empty(){"blocked"}else{"ready"},"request_completed":schema.is_some()||preset_review_ready,"task_completed":false},
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
            "result_diagnostics":result_diagnostics,
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
        projection["sample_review"] = json!({
            "pending_count":pending_sample_reviews.len(),
            "pending_request_ids":pending_sample_reviews.iter().map(|request|request.input.id).collect::<Vec<_>>(),
            "sample_test_id":processing_candidate.as_ref().map(|sample|sample["sample_test_id"].clone()),
            "url":format!("{root}/visual-selections")
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
        if formal_result.is_object() || !pending_sample_reviews.is_empty() {
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
    use super::{
        authorization_result_diagnostic, call_result_diagnostic, journey_sample_operation,
        sample_result_diagnostics,
    };
    use annotagent_core::{ModelFailure, ModelFailureCategory, ModelFailureStage};
    use annotagent_storage::{
        ConversationCallBudget, ConversationCallGrant, ConversationCallReceipt,
        ConversationCallStatus, SampleOperation,
    };
    use serde_json::json;
    use uuid::Uuid;

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

    #[test]
    fn call_diagnostics_separate_not_sent_unknown_and_invalid_structure() {
        let id = Uuid::new_v4();
        let receipt = |status, failure| ConversationCallReceipt {
            id,
            task_id: Uuid::new_v4(),
            request_hash: "a".repeat(64),
            status,
            evidence: None,
            started_at: Some("2026-09-12T00:00:00Z".into()),
            completed_at: Some("2026-09-12T00:00:01Z".into()),
            duration_ms: Some(1_000),
            stage: Some("settled".into()),
            failure,
        };
        let not_sent = call_result_diagnostic(
            &receipt(
                ConversationCallStatus::Failed,
                Some(ModelFailure {
                    stage: ModelFailureStage::PrepareRequest,
                    category: ModelFailureCategory::Configuration,
                    http_status: None,
                }),
            ),
            "/calls",
        )
        .unwrap();
        assert_eq!(not_sent["code"], "provider_request_not_sent");
        assert_eq!(not_sent["provider_received"], false);
        assert_eq!(not_sent["automatic_retry"], false);

        let invalid = call_result_diagnostic(
            &receipt(
                ConversationCallStatus::Failed,
                Some(ModelFailure {
                    stage: ModelFailureStage::StructuredOutput,
                    category: ModelFailureCategory::InvalidStructuredOutput,
                    http_status: None,
                }),
            ),
            "/calls",
        )
        .unwrap();
        assert_eq!(invalid["code"], "model_response_invalid_structure");
        assert_eq!(invalid["provider_received"], true);

        let unknown = call_result_diagnostic(
            &receipt(
                ConversationCallStatus::InDoubt,
                Some(ModelFailure {
                    stage: ModelFailureStage::ProviderRequest,
                    category: ModelFailureCategory::Interrupted,
                    http_status: None,
                }),
            ),
            "/calls",
        )
        .unwrap();
        assert_eq!(unknown["code"], "provider_outcome_unknown");
        assert!(unknown["provider_received"].is_null());
        assert_eq!(unknown["automatic_retry"], false);
    }

    #[test]
    fn sample_diagnostics_keep_legal_empty_separate_from_projection_failure() {
        let operation = SampleOperation {
            id: "TEST-sample".into(),
            project_id: "TEST-project".into(),
            draft_id: "TEST-draft".into(),
            authorization_fingerprint: "scope".into(),
            request: json!({}),
            status: "succeeded".into(),
            error: None,
            created_at: "2026-09-12T00:00:00Z".into(),
            updated_at: "2026-09-12T00:00:01Z".into(),
        };
        let report = json!({"samples":[
            {"empty":true,"failed":false,"failure_classes":["no_candidate"],
             "projection":{"final_candidates":[],"review_candidates":[]}},
            {"empty":false,"failed":true,"failure_classes":["invalid_artifact"],
             "projection":{"final_candidates":[{"outcome":{"id":"preserved"}}],"review_candidates":[]}},
            {"empty":true,"failed":true,"failure_classes":["provider_failure"],
             "projection":{"final_candidates":[],"review_candidates":[]}}
        ]});
        let diagnostics = sample_result_diagnostics(&operation, &report);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0]["code"], "legal_empty_detection");
        assert_eq!(diagnostics[0]["human_negative_recorded"], false);
        assert_eq!(diagnostics[1]["code"], "candidate_projection_failed");
        assert_eq!(diagnostics[1]["preserves_existing_results"], true);
        assert_eq!(
            report["samples"][1]["projection"]["final_candidates"][0]["outcome"]["id"],
            "preserved"
        );
    }

    #[test]
    fn authorization_diagnostics_keep_expiry_and_exhaustion_explicit() {
        let now = chrono::Utc::now();
        let grant = |expires_at, maximum_calls| ConversationCallGrant {
            id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            scope_hash: "a".repeat(64),
            maximum_calls,
            expires_at,
        };
        let expired = authorization_result_diagnostic(
            &ConversationCallBudget {
                current_grant: grant(now - chrono::Duration::seconds(1), 2),
                used_calls: 0,
                revoked: false,
            },
            "/budget",
        )
        .unwrap();
        assert_eq!(expired["code"], "authorization_expired");
        assert_eq!(expired["automatic_retry"], false);
        assert_eq!(expired["safe_action"]["url"], "/budget");

        let exhausted = authorization_result_diagnostic(
            &ConversationCallBudget {
                current_grant: grant(now + chrono::Duration::minutes(5), 1),
                used_calls: 1,
                revoked: false,
            },
            "/budget",
        )
        .unwrap();
        assert_eq!(exhausted["code"], "task_call_budget_exhausted");
        assert_eq!(exhausted["scope"]["maximum_calls"], 1);
        assert_eq!(exhausted["scope"]["used_calls"], 1);
        assert_eq!(exhausted["preserves_existing_results"], true);
    }
}
