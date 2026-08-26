use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Instant};

use annotagent_core::{
    AdditionalUsage, AgentTool, Annotation, AnnotationId, AnnotationProvenance, AnnotationRevision,
    AnnotationRevisionId, AnnotationSource, AnnotationValue, Budget, CoreError, DomainSkill,
    ImageFrame, ImageId, IssueSeverity, LabelId, ModelImage, ModelMessage, ModelRequest, ModelRole,
    PricingConfig, ProjectSchema, RefinementContext, ReviewDecision, ReviewStatus, RevisionActor,
    RunEvent, RunEventKind, RunEventPayload, RunId, RunStatus, SuggestedAction, TaskConfig,
    TaskKind, ToolContext, ToolDefinition, ToolResult, UsageRecord, UsageTotals, ValidationContext,
    ValidationEvidence, ValidationIssue, VisionModelProvider,
};
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};
use thiserror::Error;
use tokio::sync::{Mutex, broadcast};

use crate::{
    ContextManager, RunControl, RunRecord, RuntimeStore, ToolRegistry, normalized_tool_signature,
};

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("runtime store error: {0}")]
    Store(String),
    #[error("run control error: {0}")]
    Control(String),
    #[error("invalid workflow: {0}")]
    Workflow(String),
    #[error("provider failed: {0}")]
    Provider(String),
    #[error("tool failed: {0}")]
    Tool(String),
    #[error("candidate is invalid: {0}")]
    Candidate(String),
    #[error("skill extension failed: {0}")]
    Skill(String),
}

#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    pub model: String,
    pub max_steps_per_image: u32,
    pub max_retries_per_task: u32,
    pub max_provider_failures: u32,
    pub max_output_tokens: u32,
    pub temperature: f32,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            model: "mock-vision".to_owned(),
            max_steps_per_image: 10,
            max_retries_per_task: 3,
            max_provider_failures: 3,
            max_output_tokens: 4096,
            temperature: 0.1,
        }
    }
}

pub struct ImageRunRequest {
    pub run_id: RunId,
    pub project_id: annotagent_core::ProjectId,
    pub project_root: PathBuf,
    pub project: Arc<ProjectSchema>,
    pub image_id: ImageId,
    pub image: Arc<ImageFrame>,
    pub model_image: Option<ModelImage>,
}

#[derive(Debug, Clone)]
pub struct ImageRunResult {
    pub run_id: RunId,
    pub committed: Vec<Annotation>,
    pub review_queue: Vec<Annotation>,
    pub issues: Vec<ValidationIssue>,
    pub usage: UsageTotals,
    pub status: RunStatus,
}

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<RunEvent>,
}

impl EventBus {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<RunEvent> {
        self.sender.subscribe()
    }

    fn send(&self, event: RunEvent) {
        let _ignored = self.sender.send(event);
    }
}

pub struct AgentRuntime {
    skill: Arc<dyn DomainSkill>,
    provider: Arc<dyn VisionModelProvider>,
    store: Arc<dyn RuntimeStore>,
    pricing: PricingConfig,
    budget: Budget,
    config: AgentLoopConfig,
    control: RunControl,
    event_bus: EventBus,
    usage: Mutex<UsageTotals>,
}

impl AgentRuntime {
    #[must_use]
    pub fn new(
        skill: Arc<dyn DomainSkill>,
        provider: Arc<dyn VisionModelProvider>,
        store: Arc<dyn RuntimeStore>,
        pricing: PricingConfig,
        budget: Budget,
        config: AgentLoopConfig,
    ) -> Self {
        Self {
            skill,
            provider,
            store,
            pricing,
            budget,
            config,
            control: RunControl::new(),
            event_bus: EventBus::new(512),
            usage: Mutex::new(UsageTotals::default()),
        }
    }

    #[must_use]
    pub fn control(&self) -> RunControl {
        self.control.clone()
    }

    #[must_use]
    pub fn event_bus(&self) -> EventBus {
        self.event_bus.clone()
    }

    pub async fn run_image(
        &self,
        request: ImageRunRequest,
    ) -> Result<ImageRunResult, RuntimeError> {
        let mut tools = ToolRegistry::new();
        for tool in generic_tools() {
            tools
                .register(tool)
                .map_err(|error| RuntimeError::Tool(error.to_string()))?;
        }
        for tool in self.skill.tool_factories() {
            tools
                .register(tool)
                .map_err(|error| RuntimeError::Tool(error.to_string()))?;
        }
        let workflow_graph = self.skill.workflow();
        let workflow = workflow_graph
            .topological_order()
            .map_err(RuntimeError::Workflow)?;
        let run = RunRecord {
            id: request.run_id,
            project_name: request.project.project.name.clone(),
            skill_id: self.skill.id().to_owned(),
            provider: self.provider.name().to_owned(),
            model: self.config.model.clone(),
            status: RunStatus::Pending,
            project_schema_json: serde_json::to_string(request.project.as_ref())
                .map_err(|error| RuntimeError::Store(error.to_string()))?,
        };
        self.store
            .create_run(&run)
            .await
            .map_err(RuntimeError::Store)?;
        self.publish(RunEvent::new(
            request.run_id,
            RunEventKind::RunCreated,
            RunEventPayload::State {
                from: None,
                to: RunStatus::Pending,
                reason: None,
            },
        ))
        .await?;
        let previous = self
            .control
            .transition(RunStatus::Running)
            .map_err(|error| RuntimeError::Control(error.to_string()))?;
        self.store
            .set_run_status(request.run_id, RunStatus::Running, None)
            .await
            .map_err(RuntimeError::Store)?;
        self.publish(RunEvent::new(
            request.run_id,
            RunEventKind::RunStarted,
            RunEventPayload::State {
                from: Some(previous),
                to: RunStatus::Running,
                reason: None,
            },
        ))
        .await?;

        let mut committed = Vec::new();
        let mut review_queue = Vec::new();
        let mut all_issues = Vec::new();
        let mut failed_tasks = std::collections::BTreeSet::new();
        for task_id in workflow {
            if self.control.cancellation_token().is_cancelled() {
                break;
            }
            let Some(task) = request.project.tasks.iter().find(|task| task.id == task_id) else {
                continue;
            };
            let dependencies = workflow_graph
                .nodes
                .iter()
                .find(|node| node.id == task_id)
                .map_or(&[][..], |node| node.depends_on.as_slice());
            if let Some(dependency) = dependencies
                .iter()
                .find(|dependency| failed_tasks.contains(*dependency))
            {
                all_issues.push(runtime_issue(
                    "dependency_failed",
                    &format!(
                        "task {} was skipped because dependency {} did not produce a candidate",
                        task.id, dependency
                    ),
                ));
                failed_tasks.insert(task.id.clone());
                continue;
            }
            let outcome = match self.run_task(&request, task, &committed, &tools).await {
                Ok(outcome) => outcome,
                Err(error) => {
                    self.finish_run_with_reason(
                        request.run_id,
                        RunStatus::Failed,
                        &format!("runtime error: {error}"),
                    )
                    .await?;
                    return Err(error);
                }
            };
            if outcome.committed.is_empty() && outcome.review_queue.is_empty() {
                failed_tasks.insert(task.id.clone());
            }
            committed.extend(outcome.committed);
            review_queue.extend(outcome.review_queue);
            all_issues.extend(outcome.issues);
        }

        let status = match self
            .control
            .status()
            .map_err(|error| RuntimeError::Control(error.to_string()))?
        {
            RunStatus::BudgetExceeded => RunStatus::BudgetExceeded,
            RunStatus::Cancelled => RunStatus::Cancelled,
            _ if !failed_tasks.is_empty() => RunStatus::Failed,
            _ if !review_queue.is_empty() => RunStatus::AwaitingReview,
            _ => RunStatus::Completed,
        };
        if !matches!(status, RunStatus::BudgetExceeded) {
            self.finish_run(request.run_id, status).await?;
        }
        Ok(ImageRunResult {
            run_id: request.run_id,
            committed,
            review_queue,
            issues: all_issues,
            usage: self.usage.lock().await.clone(),
            status,
        })
    }

    async fn run_task(
        &self,
        request: &ImageRunRequest,
        task: &TaskConfig,
        related: &[Annotation],
        tools: &ToolRegistry,
    ) -> Result<TaskOutcome, RuntimeError> {
        self.publish(
            RunEvent::new(
                request.run_id,
                RunEventKind::TaskStarted,
                RunEventPayload::Message {
                    summary: format!("started task {}", task.id),
                },
            )
            .scoped(Some(request.image_id), Some(task.id.clone())),
        )
        .await?;
        let mut definitions = tools.definitions_for_task(&task.id);
        if !task.labels.is_empty()
            && let Some(submit) = definitions
                .iter_mut()
                .find(|definition| definition.name == "submit_annotation_candidates")
            && let Some(label_schema) = submit
                .parameters
                .pointer_mut("/properties/annotations/items/properties/label")
        {
            label_schema["enum"] = serde_json::json!(task.labels);
        }
        let initial_usage = self.usage.lock().await.clone();
        let mut messages = ContextManager::build(
            self.skill.as_ref(),
            &request.project,
            task,
            &request.image.metadata,
            &definitions,
            &initial_usage,
            self.config.max_steps_per_image,
        )
        .map_err(|error| RuntimeError::Skill(error.to_string()))?;
        let mut retries = 0;
        let mut provider_failures = 0;
        let mut previous_signature = None;
        let mut repeated = 0_u32;
        let mut outcome = TaskOutcome::default();
        let mut evidence_calls = 0_u32;
        for step in 0..self.config.max_steps_per_image {
            self.control
                .wait_until_runnable()
                .await
                .map_err(|error| RuntimeError::Control(error.to_string()))?;
            if self.control.cancellation_token().is_cancelled() {
                return Ok(outcome);
            }
            let budget_reason = {
                let usage = self.usage.lock().await;
                self.budget.exceeded_by(&usage)
            };
            if let Some(reason) = budget_reason {
                self.finish_run_with_reason(request.run_id, RunStatus::BudgetExceeded, &reason)
                    .await?;
                return Ok(outcome);
            }
            self.publish(
                RunEvent::new(
                    request.run_id,
                    RunEventKind::ModelCallStarted,
                    RunEventPayload::Progress {
                        completed_images: 0,
                        total_images: 1,
                        current_step: step + 1,
                        max_steps: self.config.max_steps_per_image,
                    },
                )
                .scoped(Some(request.image_id), Some(task.id.clone())),
            )
            .await?;
            let available_definitions = if evidence_calls == 0 {
                definitions.clone()
            } else {
                definitions
                    .iter()
                    .filter(|definition| is_terminal_tool(&definition.name))
                    .cloned()
                    .collect()
            };
            let model_request = ModelRequest {
                model: self.config.model.clone(),
                task_id: task.id.clone(),
                messages: messages.clone(),
                images: request.model_image.clone().into_iter().collect(),
                tools: available_definitions,
                max_output_tokens: self.config.max_output_tokens,
                temperature: self.config.temperature,
                extra: BTreeMap::new(),
            };
            let started_at = Utc::now();
            let started = Instant::now();
            let response = self
                .provider
                .complete(model_request, self.control.cancellation_token())
                .await;
            let response = match response {
                Ok(response) => {
                    provider_failures = 0;
                    response
                }
                Err(error) => {
                    provider_failures += 1;
                    if provider_failures >= self.config.max_provider_failures {
                        return Err(RuntimeError::Provider(error.to_string()));
                    }
                    messages.push(ModelMessage {
                        role: ModelRole::User,
                        content: format!("Provider call failed safely; retry: {error}"),
                        tool_call_id: None,
                    });
                    continue;
                }
            };
            let additional = AdditionalUsage {
                image_count: u64::from(request.model_image.is_some()),
                request_count: 1,
                ..AdditionalUsage::default()
            };
            let usage_record = UsageRecord {
                provider: self.provider.name().to_owned(),
                model: self.config.model.clone(),
                endpoint_summary: self.provider.name().to_owned(),
                started_at,
                completed_at: Utc::now(),
                duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                tokens: response.usage.clone(),
                additional: additional.clone(),
                request_id: response.request_id.clone(),
                cost: self.pricing.calculate(&response.usage, &additional),
                success: true,
                retry_count: provider_failures,
            };
            self.store
                .record_usage(request.run_id, &usage_record)
                .await
                .map_err(RuntimeError::Store)?;
            {
                let mut totals = self.usage.lock().await;
                totals.add(&usage_record);
                self.publish(
                    RunEvent::new(
                        request.run_id,
                        RunEventKind::UsageUpdated,
                        RunEventPayload::Usage {
                            totals: totals.clone(),
                        },
                    )
                    .scoped(Some(request.image_id), Some(task.id.clone())),
                )
                .await?;
            }
            self.publish(
                RunEvent::new(
                    request.run_id,
                    RunEventKind::ModelCallCompleted,
                    RunEventPayload::Message {
                        summary: format!(
                            "model returned {} tool call(s)",
                            response.tool_calls.len()
                        ),
                    },
                )
                .scoped(Some(request.image_id), Some(task.id.clone())),
            )
            .await?;
            if let Some(content) = response.content {
                messages.push(ModelMessage {
                    role: ModelRole::Assistant,
                    content,
                    tool_call_id: None,
                });
            }
            if response.tool_calls.is_empty() {
                break;
            }

            let mut retry_requested = false;
            'tool_calls: for call in response.tool_calls {
                let signature = normalized_tool_signature(&call.name, &call.arguments);
                if previous_signature.as_ref() == Some(&signature) {
                    repeated += 1;
                } else {
                    repeated = 1;
                    previous_signature = Some(signature);
                }
                if repeated >= 3 {
                    outcome.issues.push(runtime_issue(
                        "repeated_tool_call",
                        "model repeated the same normalized tool call three times",
                    ));
                    return Ok(outcome);
                }
                if !is_terminal_tool(&call.name) && evidence_calls >= 1 {
                    let message = "runtime skipped the tool call because this task already used its one evidence/refinement call";
                    self.store
                        .record_tool_call(
                            request.run_id,
                            &call.id,
                            &call.name,
                            &call.arguments,
                            None,
                            Some(message),
                        )
                        .await
                        .map_err(RuntimeError::Store)?;
                    messages.push(ModelMessage {
                        role: ModelRole::Tool,
                        content: format!(
                            "{message}; submit the final candidate now with submit_annotation_candidates"
                        ),
                        tool_call_id: Some(call.id.clone()),
                    });
                    self.publish(
                        RunEvent::new(
                            request.run_id,
                            RunEventKind::ToolCallCompleted,
                            RunEventPayload::Tool {
                                call_id: call.id,
                                name: call.name,
                                summary: message.to_owned(),
                                success: false,
                            },
                        )
                        .scoped(Some(request.image_id), Some(task.id.clone())),
                    )
                    .await?;
                    if !outcome
                        .issues
                        .iter()
                        .any(|issue| issue.code == "tool_call_budget_exceeded")
                    {
                        outcome.issues.push(runtime_issue(
                            "tool_call_budget_exceeded",
                            "model requested more than one evidence/refinement call for a task",
                        ));
                    }
                    continue;
                }
                if !is_terminal_tool(&call.name) {
                    evidence_calls += 1;
                }
                self.publish(
                    RunEvent::new(
                        request.run_id,
                        RunEventKind::ToolCallStarted,
                        RunEventPayload::Tool {
                            call_id: call.id.clone(),
                            name: call.name.clone(),
                            summary: "arguments validated by registry".to_owned(),
                            success: true,
                        },
                    )
                    .scoped(Some(request.image_id), Some(task.id.clone())),
                )
                .await?;
                let context = ToolContext {
                    project_root: request.project_root.clone(),
                    run_id: request.run_id,
                    image_id: Some(request.image_id),
                    image: Some(request.image.clone()),
                    task_id: Some(task.id.clone()),
                    cancellation: self.control.cancellation_token(),
                };
                let result = tools
                    .execute(&call.name, &context, call.arguments.clone())
                    .await;
                match result {
                    Ok(result) => {
                        self.store
                            .record_tool_call(
                                request.run_id,
                                &call.id,
                                &call.name,
                                &call.arguments,
                                Some(&result),
                                None,
                            )
                            .await
                            .map_err(RuntimeError::Store)?;
                        self.publish(
                            RunEvent::new(
                                request.run_id,
                                RunEventKind::ToolCallCompleted,
                                RunEventPayload::Tool {
                                    call_id: call.id.clone(),
                                    name: call.name.clone(),
                                    summary: result.summary.clone(),
                                    success: true,
                                },
                            )
                            .scoped(Some(request.image_id), Some(task.id.clone())),
                        )
                        .await?;
                        messages.push(ModelMessage {
                            role: ModelRole::Tool,
                            content: result.summary.clone(),
                            tool_call_id: Some(call.id.clone()),
                        });
                        if call.name == "submit_annotation_candidates" {
                            let candidates = match parse_candidates(
                                &call.arguments,
                                request.image_id,
                                task,
                                self.provider.name(),
                                &self.config.model,
                            ) {
                                Ok(candidates) => candidates,
                                Err(error) => {
                                    let message = error.to_string();
                                    outcome
                                        .issues
                                        .push(runtime_issue("invalid_candidate", &message));
                                    messages.push(ModelMessage {
                                        role: ModelRole::User,
                                        content: format!(
                                            "Candidate rejected before validation: {message}. Correct the label/value shape and submit again."
                                        ),
                                        tool_call_id: None,
                                    });
                                    retry_requested = true;
                                    retries += 1;
                                    break 'tool_calls;
                                }
                            };
                            let decision = self
                                .process_candidates(request, task, related, candidates, retries)
                                .await?;
                            let issue_summary = decision
                                .issues
                                .iter()
                                .map(|issue| format!("{}: {}", issue.code, issue.message))
                                .collect::<Vec<_>>()
                                .join("; ");
                            outcome.issues.extend(decision.issues);
                            outcome.committed.extend(decision.committed);
                            outcome.review_queue.extend(decision.review_queue);
                            if decision.retry {
                                messages.push(ModelMessage {
                                    role: ModelRole::User,
                                    content: format!(
                                        "Deterministic validator issues require correction: {issue_summary}"
                                    ),
                                    tool_call_id: None,
                                });
                                retry_requested = true;
                                retries += 1;
                                break 'tool_calls;
                            }
                            return Ok(outcome);
                        } else if call.name == "request_human_review" {
                            outcome.issues.push(runtime_issue(
                                "model_requested_review",
                                "model explicitly requested human review",
                            ));
                            return Ok(outcome);
                        } else if call.name == "finish_task" {
                            return Ok(outcome);
                        }
                    }
                    Err(error) => {
                        let message = error.to_string();
                        self.store
                            .record_tool_call(
                                request.run_id,
                                &call.id,
                                &call.name,
                                &call.arguments,
                                None,
                                Some(&message),
                            )
                            .await
                            .map_err(RuntimeError::Store)?;
                        self.publish(
                            RunEvent::new(
                                request.run_id,
                                RunEventKind::ToolCallCompleted,
                                RunEventPayload::Tool {
                                    call_id: call.id.clone(),
                                    name: call.name.clone(),
                                    summary: message.clone(),
                                    success: false,
                                },
                            )
                            .scoped(Some(request.image_id), Some(task.id.clone())),
                        )
                        .await?;
                        messages.push(ModelMessage {
                            role: ModelRole::Tool,
                            content: format!("tool rejected: {message}"),
                            tool_call_id: Some(call.id),
                        });
                        if call.name == "submit_annotation_candidates" {
                            outcome
                                .issues
                                .push(runtime_issue("invalid_candidate", &message));
                            retry_requested = true;
                            retries += 1;
                            break 'tool_calls;
                        }
                    }
                }
            }
            if retry_requested {
                self.publish(
                    RunEvent::new(
                        request.run_id,
                        RunEventKind::RetryScheduled,
                        RunEventPayload::Message {
                            summary: format!("task retry {retries}"),
                        },
                    )
                    .scoped(Some(request.image_id), Some(task.id.clone())),
                )
                .await?;
            }
        }
        if outcome.committed.is_empty() && outcome.review_queue.is_empty() {
            outcome.issues.push(runtime_issue(
                "max_steps_or_no_submission",
                "task ended without a committed candidate",
            ));
        }
        Ok(outcome)
    }

    async fn process_candidates(
        &self,
        request: &ImageRunRequest,
        task: &TaskConfig,
        related: &[Annotation],
        candidates: Vec<Annotation>,
        retries: u32,
    ) -> Result<CandidateDecision, RuntimeError> {
        let validators = self.skill.validators();
        let refiners = self.skill.refiners();
        let mut output = CandidateDecision::default();
        let peer_candidates = candidates.clone();
        for mut candidate in candidates {
            let correction_risk = self
                .store
                .correction_risk(
                    request.project_id,
                    self.skill.id(),
                    &task.id,
                    candidate.label.as_ref(),
                )
                .await
                .map_err(RuntimeError::Store)?;
            let mut refiner_confidence = None;
            for refiner_id in &task.refiners {
                let refiner = refiners
                    .iter()
                    .find(|refiner| refiner.id() == refiner_id)
                    .ok_or_else(|| {
                        RuntimeError::Skill(format!("unknown refiner {refiner_id:?}"))
                    })?;
                self.publish(
                    RunEvent::new(
                        request.run_id,
                        RunEventKind::RefinementStarted,
                        RunEventPayload::Annotation {
                            annotation_ids: vec![candidate.id],
                            summary: format!("running refiner {refiner_id}"),
                        },
                    )
                    .scoped(Some(request.image_id), Some(task.id.clone())),
                )
                .await?;
                let before = candidate.snapshot();
                let result = refiner
                    .refine(&RefinementContext {
                        project: &request.project,
                        image: &request.image,
                        candidate: &candidate,
                        related_annotations: related,
                    })
                    .map_err(|error| RuntimeError::Skill(error.to_string()))?;
                candidate = result.annotation;
                candidate.source = AnnotationSource::ModelAndTool;
                refiner_confidence = Some(result.confidence);
                self.store
                    .record_revision(&AnnotationRevision {
                        revision_id: AnnotationRevisionId::new(),
                        annotation_id: candidate.id,
                        parent_revision_id: None,
                        before: Some(before),
                        after: Some(candidate.snapshot()),
                        actor: RevisionActor::Runtime,
                        reason: Some(format!("deterministic refiner {refiner_id}")),
                        created_at: Utc::now(),
                    })
                    .await
                    .map_err(RuntimeError::Store)?;
                self.publish(
                    RunEvent::new(
                        request.run_id,
                        RunEventKind::RefinementCompleted,
                        RunEventPayload::Annotation {
                            annotation_ids: vec![candidate.id],
                            summary: result.summary,
                        },
                    )
                    .scoped(Some(request.image_id), Some(task.id.clone())),
                )
                .await?;
                output.issues.extend(result.issues);
            }
            let validation_related: Vec<Annotation> = related
                .iter()
                .chain(
                    peer_candidates
                        .iter()
                        .filter(|peer| peer.id != candidate.id),
                )
                .cloned()
                .collect();
            let mut issues = Vec::new();
            for validator_id in &task.validators {
                let validator = validators
                    .iter()
                    .find(|validator| validator.id() == validator_id)
                    .ok_or_else(|| {
                        RuntimeError::Skill(format!("unknown validator {validator_id:?}"))
                    })?;
                issues.extend(
                    validator
                        .validate(&ValidationContext {
                            project: &request.project,
                            image: Some(&request.image),
                            candidate: &candidate,
                            related_annotations: &validation_related,
                            correction_risk,
                        })
                        .map_err(|error| RuntimeError::Skill(error.to_string()))?,
                );
            }
            self.store
                .record_validation(request.run_id, &issues)
                .await
                .map_err(RuntimeError::Store)?;
            self.publish(
                RunEvent::new(
                    request.run_id,
                    RunEventKind::ValidationCompleted,
                    RunEventPayload::Validation {
                        issue_codes: issues.iter().map(|issue| issue.code.clone()).collect(),
                        accepted: !issues
                            .iter()
                            .any(|issue| issue.severity == IssueSeverity::Error),
                    },
                )
                .scoped(Some(request.image_id), Some(task.id.clone())),
            )
            .await?;
            let decision = self
                .skill
                .review_policy()
                .decide(&annotagent_core::ReviewContext {
                    annotation: &candidate,
                    issues: &issues,
                    refiner_confidence,
                    correction_risk,
                    evidence_conflict: false,
                    retry_count: retries,
                    max_retries: self.config.max_retries_per_task,
                });
            output.issues.extend(issues);
            match decision {
                ReviewDecision::AutoAccept { reasons } => {
                    candidate.review_status = ReviewStatus::AutoAccepted;
                    self.store
                        .commit_annotation(request.run_id, &candidate)
                        .await
                        .map_err(RuntimeError::Store)?;
                    self.publish(
                        RunEvent::new(
                            request.run_id,
                            RunEventKind::AnnotationCommitted,
                            RunEventPayload::Annotation {
                                annotation_ids: vec![candidate.id],
                                summary: reasons.join("; "),
                            },
                        )
                        .scoped(Some(request.image_id), Some(task.id.clone())),
                    )
                    .await?;
                    output.committed.push(candidate);
                }
                ReviewDecision::Retry { reasons } => {
                    output.retry = true;
                    output.issues.push(runtime_issue(
                        "policy_retry",
                        &format!("retry requested: {}", reasons.join("; ")),
                    ));
                }
                ReviewDecision::HumanReview { reasons } => {
                    candidate.review_status = ReviewStatus::NeedsReview;
                    self.store
                        .commit_annotation(request.run_id, &candidate)
                        .await
                        .map_err(RuntimeError::Store)?;
                    self.publish(
                        RunEvent::new(
                            request.run_id,
                            RunEventKind::ReviewRequested,
                            RunEventPayload::Annotation {
                                annotation_ids: vec![candidate.id],
                                summary: reasons.join("; "),
                            },
                        )
                        .scoped(Some(request.image_id), Some(task.id.clone())),
                    )
                    .await?;
                    output.review_queue.push(candidate);
                }
                ReviewDecision::Reject { reasons } => {
                    output.issues.push(runtime_issue(
                        "policy_rejected",
                        &format!("candidate rejected: {}", reasons.join("; ")),
                    ));
                }
            }
        }
        Ok(output)
    }

    async fn publish(&self, event: RunEvent) -> Result<(), RuntimeError> {
        self.store
            .record_event(&event)
            .await
            .map_err(RuntimeError::Store)?;
        self.event_bus.send(event);
        Ok(())
    }

    async fn finish_run(&self, run_id: RunId, status: RunStatus) -> Result<(), RuntimeError> {
        self.finish_run_with_reason(run_id, status, "run reached a terminal condition")
            .await
    }

    async fn finish_run_with_reason(
        &self,
        run_id: RunId,
        status: RunStatus,
        reason: &str,
    ) -> Result<(), RuntimeError> {
        let previous = self
            .control
            .status()
            .map_err(|error| RuntimeError::Control(error.to_string()))?;
        if previous != status {
            self.control
                .transition(status)
                .map_err(|error| RuntimeError::Control(error.to_string()))?;
        }
        self.store
            .set_run_status(run_id, status, Some(reason))
            .await
            .map_err(RuntimeError::Store)?;
        let kind = match status {
            RunStatus::Completed => RunEventKind::RunCompleted,
            RunStatus::Cancelled => RunEventKind::RunCancelled,
            RunStatus::BudgetExceeded => RunEventKind::RunBudgetExceeded,
            RunStatus::AwaitingReview => RunEventKind::ReviewRequested,
            _ => RunEventKind::RunFailed,
        };
        self.publish(RunEvent::new(
            run_id,
            kind,
            RunEventPayload::State {
                from: Some(previous),
                to: status,
                reason: Some(reason.to_owned()),
            },
        ))
        .await
    }
}

#[derive(Default)]
struct TaskOutcome {
    committed: Vec<Annotation>,
    review_queue: Vec<Annotation>,
    issues: Vec<ValidationIssue>,
}

#[derive(Default)]
struct CandidateDecision {
    committed: Vec<Annotation>,
    review_queue: Vec<Annotation>,
    issues: Vec<ValidationIssue>,
    retry: bool,
}

#[derive(Debug, Deserialize)]
struct CandidateSubmission {
    annotations: Vec<CandidateAnnotation>,
}

#[derive(Debug, Deserialize)]
struct CandidateAnnotation {
    label: Option<LabelId>,
    value: AnnotationValue,
    #[serde(default)]
    attributes: BTreeMap<String, annotagent_core::AttributeValue>,
    confidence: Option<f32>,
}

fn parse_candidates(
    value: &Value,
    image_id: ImageId,
    task: &TaskConfig,
    provider: &str,
    model: &str,
) -> Result<Vec<Annotation>, RuntimeError> {
    let submission: CandidateSubmission = serde_json::from_value(value.clone())
        .map_err(|error| RuntimeError::Candidate(error.to_string()))?;
    submission
        .annotations
        .into_iter()
        .map(|candidate| {
            if candidate.value.task_kind() != task.kind
                && !matches!(task.kind, TaskKind::Attributes)
            {
                return Err(RuntimeError::Candidate(format!(
                    "task {:?} expects {:?}, candidate contains {:?}",
                    task.id.as_str(),
                    task.kind,
                    candidate.value.task_kind()
                )));
            }
            if let Some(label) = &candidate.label
                && !task.labels.is_empty()
                && !task.labels.iter().any(|allowed| allowed == label.as_str())
            {
                return Err(RuntimeError::Candidate(format!(
                    "label {:?} is not allowed by task {:?}",
                    label.as_str(),
                    task.id.as_str()
                )));
            }
            let annotation = Annotation {
                id: AnnotationId::new(),
                image_id,
                task_id: task.id.clone(),
                label: candidate.label,
                value: candidate.value,
                attributes: candidate.attributes,
                confidence: candidate.confidence,
                source: AnnotationSource::Model,
                review_status: ReviewStatus::Draft,
                provenance: AnnotationProvenance {
                    provider: Some(provider.to_owned()),
                    model: Some(model.to_owned()),
                    tool_names: vec!["submit_annotation_candidates".to_owned()],
                    ..AnnotationProvenance::default()
                },
                created_at: Utc::now(),
            };
            annotation
                .validate()
                .map_err(|error| RuntimeError::Candidate(error.to_string()))?;
            Ok(annotation)
        })
        .collect()
}

fn runtime_issue(code: &str, message: &str) -> ValidationIssue {
    ValidationIssue {
        code: code.to_owned(),
        severity: IssueSeverity::Warning,
        annotation_ids: Vec::new(),
        message: message.to_owned(),
        suggested_action: SuggestedAction::HumanReview,
        evidence: ValidationEvidence::Rule {
            facts: BTreeMap::new(),
        },
    }
}

fn is_terminal_tool(name: &str) -> bool {
    matches!(
        name,
        "submit_annotation_candidates" | "finish_task" | "request_human_review"
    )
}

fn generic_tools() -> Vec<Arc<dyn AgentTool>> {
    vec![
        Arc::new(EchoTool::new(
            "submit_annotation_candidates",
            "Submit the final typed candidates for the CURRENT task. Geometry coordinates must be normalized to [0,1]. Bounding boxes use rect=[x,y,width,height], never [x1,y1,x2,y2].",
            json!({
                "type": "object",
                "properties": {
                    "annotations": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "label": {"type": "string"},
                                "value": {"type": "object"},
                                "attributes": {"type": "object"},
                                "confidence": {"type": "number"}
                            },
                            "required": ["value"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["annotations"],
                "additionalProperties": false
            }),
        )),
        Arc::new(EchoTool::new(
            "finish_task",
            "Finish the current task when no more candidates are needed",
            json!({"type": "object", "properties": {}, "additionalProperties": false}),
        )),
        Arc::new(EchoTool::new(
            "request_human_review",
            "Request a human decision with an explicit reason",
            json!({
                "type": "object",
                "properties": {"reason": {"type": "string"}},
                "required": ["reason"],
                "additionalProperties": false
            }),
        )),
    ]
}

struct EchoTool {
    definition: ToolDefinition,
}

impl EchoTool {
    fn new(name: &str, description: &str, parameters: Value) -> Self {
        Self {
            definition: ToolDefinition {
                name: name.to_owned(),
                description: description.to_owned(),
                parameters,
                read_only: false,
            },
        }
    }
}

#[async_trait]
impl AgentTool for EchoTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(
        &self,
        _context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, CoreError> {
        Ok(ToolResult {
            summary: format!(
                "{} request accepted for runtime processing",
                self.definition.name
            ),
            data: arguments,
        })
    }
}

#[cfg(test)]
mod tests {
    use annotagent_core::TaskId;

    use super::*;

    #[test]
    fn candidate_parser_rejects_wrong_task_kind() {
        let task = TaskConfig {
            id: TaskId::from("scene"),
            kind: TaskKind::Classification,
            labels: vec!["normal".to_owned()],
            required: true,
            multi_label: false,
            depends_on: Vec::new(),
            validators: Vec::new(),
            refiners: Vec::new(),
            target_task: None,
            target_labels: Vec::new(),
            attributes: BTreeMap::new(),
        };
        let result = parse_candidates(
            &json!({
                "annotations": [{
                    "label": "normal",
                    "value": {"kind": "bounding_box", "rect": [0.1, 0.1, 0.2, 0.2]},
                    "confidence": 0.9
                }]
            }),
            ImageId::new(),
            &task,
            "mock",
            "mock",
        );
        assert!(result.is_err());
    }
}
