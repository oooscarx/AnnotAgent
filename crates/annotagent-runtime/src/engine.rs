use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use annotagent_core::{
    AdditionalUsage, AgentTool, Annotation, AnnotationId, AnnotationProvenance, AnnotationRevision,
    AnnotationRevisionId, AnnotationSource, AnnotationValue, ArtifactId, ArtifactProvenance,
    ArtifactRole, ArtifactValidationState, Budget, CoreError, DomainSkill, ImageFrame, ImageId,
    IssueSeverity, LabelId, ModelImage, ModelMessage, ModelRequest, ModelRole, PricingConfig,
    ProjectSchema, RefinementContext, ReviewDecision, ReviewStatus, RevisionActor, RunEvent,
    RunEventKind, RunEventPayload, RunId, RunStatus, SuggestedAction, TaskConfig, TaskId, TaskKind,
    TaskRunStatus, ToolContext, ToolDefinition, ToolResult, UsageRecord, UsageSource, UsageTotals,
    ValidationContext, ValidationEvidence, ValidationIssue, VisionArtifact, VisionArtifactValue,
    VisionModelProvider,
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
    #[error("invalid model tool-call protocol: {0}")]
    Protocol(String),
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
    pub max_model_turns_per_task: u32,
    pub max_tool_calls_per_task: u32,
    pub max_recovery_turns_per_task: u32,
    pub task_timeout: Duration,
    pub provider_request_timeout: Duration,
    pub max_retries: u32,
    pub max_output_tokens: u32,
    pub temperature: f32,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            model: "mock-vision".to_owned(),
            max_model_turns_per_task: 8,
            max_tool_calls_per_task: 12,
            max_recovery_turns_per_task: 2,
            task_timeout: Duration::from_secs(300),
            provider_request_timeout: Duration::from_secs(120),
            max_retries: 3,
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
    workflow_snapshot_json: Option<String>,
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
            workflow_snapshot_json: None,
        }
    }

    #[must_use]
    pub fn with_workflow_snapshot_json(mut self, snapshot: Option<String>) -> Self {
        self.workflow_snapshot_json = snapshot;
        self
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
            project_id: request.project_id,
            project_name: request.project.project.name.clone(),
            skill_id: self.skill.id().to_owned(),
            provider: self.provider.name().to_owned(),
            model: self.config.model.clone(),
            status: RunStatus::Pending,
            project_schema_json: serde_json::to_string(request.project.as_ref())
                .map_err(|error| RuntimeError::Store(error.to_string()))?,
            workflow_snapshot_json: self.workflow_snapshot_json.clone(),
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
        let mut required_failures = std::collections::BTreeSet::new();
        let mut optional_failures = std::collections::BTreeSet::new();
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
                if task.required {
                    required_failures.insert(task.id.clone());
                } else {
                    optional_failures.insert(task.id.clone());
                }
                self.complete_task(
                    &request,
                    task,
                    TaskRunStatus::Skipped,
                    Some(&format!("dependency {dependency} failed")),
                )
                .await?;
                continue;
            }
            let task_started = Instant::now();
            let outcome = match tokio::time::timeout(
                self.config.task_timeout,
                self.run_task(&request, task, &committed, &tools),
            )
            .await
            {
                Ok(Ok(outcome)) => outcome,
                Err(_) => {
                    let elapsed_ms =
                        u64::try_from(task_started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let message = format!(
                        "task timeout: task={} node={} elapsed_ms={} code=task_timeout",
                        task.id, task.id, elapsed_ms
                    );
                    all_issues.push(runtime_issue("task_timeout", &message));
                    self.publish_task_failure(&request, task, elapsed_ms, "task_timeout", &message)
                        .await?;
                    failed_tasks.insert(task.id.clone());
                    if task.required {
                        required_failures.insert(task.id.clone());
                    } else {
                        optional_failures.insert(task.id.clone());
                    }
                    self.complete_task(&request, task, TaskRunStatus::Failed, Some(&message))
                        .await?;
                    continue;
                }
                Ok(Err(error)) => {
                    let elapsed_ms =
                        u64::try_from(task_started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let message = format!(
                        "task failed: task={} node={} elapsed_ms={} code=task_runtime_failed error={error}",
                        task.id, task.id, elapsed_ms
                    );
                    all_issues.push(runtime_issue("task_runtime_failed", &message));
                    self.publish_task_failure(
                        &request,
                        task,
                        elapsed_ms,
                        "task_runtime_failed",
                        &message,
                    )
                    .await?;
                    failed_tasks.insert(task.id.clone());
                    if task.required {
                        required_failures.insert(task.id.clone());
                    } else {
                        optional_failures.insert(task.id.clone());
                    }
                    self.complete_task(&request, task, TaskRunStatus::Failed, Some(&message))
                        .await?;
                    continue;
                }
            };
            let task_status = if !outcome.review_queue.is_empty() {
                TaskRunStatus::NeedsReview
            } else if !outcome.committed.is_empty() {
                TaskRunStatus::Succeeded
            } else if outcome.succeeded_empty {
                TaskRunStatus::SucceededEmpty
            } else if self.control.cancellation_token().is_cancelled() {
                TaskRunStatus::Cancelled
            } else {
                TaskRunStatus::Failed
            };
            if task_status == TaskRunStatus::Failed {
                failed_tasks.insert(task.id.clone());
                if task.required {
                    required_failures.insert(task.id.clone());
                } else {
                    optional_failures.insert(task.id.clone());
                }
            }
            self.complete_task(&request, task, task_status, None)
                .await?;
            committed.extend(outcome.committed);
            review_queue.extend(outcome.review_queue);
            all_issues.extend(outcome.issues);
        }

        let operational_issues = all_issues
            .iter()
            .filter(|issue| is_operational_issue(&issue.code))
            .cloned()
            .collect::<Vec<_>>();
        if !operational_issues.is_empty() {
            self.store
                .record_validation(request.run_id, &operational_issues)
                .await
                .map_err(RuntimeError::Store)?;
        }

        let status = match self
            .control
            .status()
            .map_err(|error| RuntimeError::Control(error.to_string()))?
        {
            RunStatus::BudgetExceeded => RunStatus::BudgetExceeded,
            RunStatus::Cancelled => RunStatus::Cancelled,
            _ if !required_failures.is_empty() => RunStatus::Failed,
            _ if !optional_failures.is_empty() => RunStatus::Partial,
            _ if !review_queue.is_empty() => RunStatus::CompletedWithReview,
            _ => RunStatus::Completed,
        };
        if !matches!(status, RunStatus::BudgetExceeded) {
            let operational_detail = operational_issues
                .iter()
                .map(|issue| format!("{}: {}", issue.code, issue.message))
                .collect::<Vec<_>>()
                .join("; ");
            let append_detail = |summary: String| {
                if operational_detail.is_empty() {
                    summary
                } else {
                    format!("{summary}; details: {operational_detail}")
                }
            };
            let terminal_reason = match status {
                RunStatus::Failed => append_detail(format!(
                    "required tasks failed: {}",
                    required_failures
                        .iter()
                        .map(TaskId::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
                RunStatus::CompletedWithReview => {
                    "one or more candidates require human review".to_owned()
                }
                RunStatus::Partial => append_detail(format!(
                    "optional tasks failed: {}",
                    optional_failures
                        .iter()
                        .map(TaskId::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
                RunStatus::Completed => {
                    "all configured tasks succeeded, including valid empty results".to_owned()
                }
                RunStatus::Cancelled => "run cancelled by user".to_owned(),
                _ => "run reached a terminal condition".to_owned(),
            };
            self.finish_run_with_reason(request.run_id, status, &terminal_reason)
                .await?;
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
        self.store
            .set_task_run_status(
                request.run_id,
                request.image_id,
                &task.id,
                TaskRunStatus::Running,
                None,
            )
            .await
            .map_err(RuntimeError::Store)?;
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
            self.config.max_model_turns_per_task,
        )
        .map_err(|error| RuntimeError::Skill(error.to_string()))?;
        for message in &messages {
            self.store
                .record_model_message(
                    request.run_id,
                    Some(request.image_id),
                    Some(&task.id),
                    message,
                )
                .await
                .map_err(RuntimeError::Store)?;
        }
        let mut retries = 0;
        let mut provider_failures = 0;
        let mut outcome = TaskOutcome::default();
        let mut current_step = 0_u32;
        let step_limit = self.config.max_model_turns_per_task.max(1);
        let mut tool_calls_used = 0_u32;
        let mut recovery_turns_used = 0_u32;
        let mut deterministic_cache = HashMap::<String, ToolResult>::new();
        let mut known_artifacts = false;
        while current_step < step_limit {
            current_step += 1;
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
                        current_step,
                        max_steps: step_limit,
                    },
                )
                .scoped(Some(request.image_id), Some(task.id.clone())),
            )
            .await?;
            validate_model_message_history(&messages)?;
            let available_definitions = available_definitions(&definitions, known_artifacts);
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
            let response = tokio::time::timeout(
                self.config.provider_request_timeout,
                self.provider
                    .complete(model_request, self.control.cancellation_token()),
            )
            .await;
            let response = match response {
                Ok(Ok(response)) => response,
                Ok(Err(error)) => {
                    provider_failures += 1;
                    let elapsed_ms =
                        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let additional = AdditionalUsage {
                        image_count: u64::from(request.model_image.is_some()),
                        request_count: 1,
                        ..AdditionalUsage::default()
                    };
                    let unknown_tokens = annotagent_core::TokenUsage {
                        input_tokens: None,
                        output_tokens: None,
                        total_tokens: None,
                        source: UsageSource::Unknown,
                    };
                    let failure_record = UsageRecord {
                        provider: self.provider.name().to_owned(),
                        model: self.config.model.clone(),
                        endpoint_summary: self.provider.name().to_owned(),
                        started_at,
                        completed_at: Utc::now(),
                        duration_ms: elapsed_ms,
                        tokens: unknown_tokens.clone(),
                        additional: additional.clone(),
                        request_id: None,
                        cost: self.pricing.calculate(&unknown_tokens, &additional),
                        success: false,
                        retry_count: provider_failures,
                    };
                    self.store
                        .record_usage(request.run_id, &failure_record)
                        .await
                        .map_err(RuntimeError::Store)?;
                    {
                        let mut totals = self.usage.lock().await;
                        totals.add(&failure_record);
                    }
                    let summary = format!(
                        "provider error: task={} node={} provider={} model={} elapsed_ms={} retry={} code=provider_error error={error}",
                        task.id,
                        task.id,
                        self.provider.name(),
                        self.config.model,
                        elapsed_ms,
                        provider_failures,
                    );
                    self.publish(
                        RunEvent::new(
                            request.run_id,
                            RunEventKind::ModelCallFailed,
                            RunEventPayload::ProviderFailure {
                                task_id: task.id.clone(),
                                node_id: task.id.to_string(),
                                provider: self.provider.name().to_owned(),
                                model: self.config.model.clone(),
                                elapsed_ms,
                                retry_count: provider_failures,
                                error_code: "provider_error".to_owned(),
                                summary,
                            },
                        )
                        .scoped(Some(request.image_id), Some(task.id.clone())),
                    )
                    .await?;
                    if provider_failures > self.config.max_retries {
                        return Err(RuntimeError::Provider(error.to_string()));
                    }
                    self.publish(
                        RunEvent::new(
                            request.run_id,
                            RunEventKind::RetryScheduled,
                            RunEventPayload::Message {
                                summary: format!(
                                    "retrying provider call for agent step {current_step}"
                                ),
                            },
                        )
                        .scoped(Some(request.image_id), Some(task.id.clone())),
                    )
                    .await?;
                    current_step = current_step.saturating_sub(1);
                    continue;
                }
                Err(_) => {
                    provider_failures += 1;
                    let elapsed_ms =
                        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let additional = AdditionalUsage {
                        image_count: u64::from(request.model_image.is_some()),
                        request_count: 1,
                        ..AdditionalUsage::default()
                    };
                    let unknown_tokens = annotagent_core::TokenUsage {
                        input_tokens: None,
                        output_tokens: None,
                        total_tokens: None,
                        source: UsageSource::Unknown,
                    };
                    let failure_record = UsageRecord {
                        provider: self.provider.name().to_owned(),
                        model: self.config.model.clone(),
                        endpoint_summary: self.provider.name().to_owned(),
                        started_at,
                        completed_at: Utc::now(),
                        duration_ms: elapsed_ms,
                        tokens: unknown_tokens.clone(),
                        additional: additional.clone(),
                        request_id: None,
                        cost: self.pricing.calculate(&unknown_tokens, &additional),
                        success: false,
                        retry_count: provider_failures,
                    };
                    self.store
                        .record_usage(request.run_id, &failure_record)
                        .await
                        .map_err(RuntimeError::Store)?;
                    self.usage.lock().await.add(&failure_record);
                    let message = format!(
                        "provider timeout: task={} node={} provider={} model={} elapsed_ms={} retry={} code=provider_timeout",
                        task.id,
                        task.id,
                        self.provider.name(),
                        self.config.model,
                        elapsed_ms,
                        provider_failures,
                    );
                    self.publish(
                        RunEvent::new(
                            request.run_id,
                            RunEventKind::ModelCallFailed,
                            RunEventPayload::ProviderFailure {
                                task_id: task.id.clone(),
                                node_id: task.id.to_string(),
                                provider: self.provider.name().to_owned(),
                                model: self.config.model.clone(),
                                elapsed_ms,
                                retry_count: provider_failures,
                                error_code: "provider_timeout".to_owned(),
                                summary: message.clone(),
                            },
                        )
                        .scoped(Some(request.image_id), Some(task.id.clone())),
                    )
                    .await?;
                    if provider_failures > self.config.max_retries {
                        return Err(RuntimeError::Provider(message));
                    }
                    current_step = current_step.saturating_sub(1);
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
            provider_failures = 0;
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
            let response_content = response.content.unwrap_or_default();
            let response_tool_calls = response.tool_calls;
            if !response_content.is_empty() || !response_tool_calls.is_empty() {
                self.append_model_message(
                    request,
                    task,
                    &mut messages,
                    ModelMessage {
                        role: ModelRole::Assistant,
                        content: response_content,
                        tool_call_id: None,
                        tool_calls: response_tool_calls.clone(),
                    },
                )
                .await?;
            }
            if response_tool_calls.is_empty() {
                self.append_model_message(
                    request,
                    task,
                    &mut messages,
                    ModelMessage {
                        role: ModelRole::User,
                        content:
                            "A registered action tool call is required; do not answer with prose."
                                .to_owned(),
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                    },
                )
                .await?;
                recovery_turns_used += 1;
                if recovery_turns_used > self.config.max_recovery_turns_per_task {
                    outcome.issues.push(runtime_issue(
                        "recovery_turn_budget_exceeded",
                        "model did not select an action before the recovery budget was exhausted",
                    ));
                    break;
                }
                self.publish(
                    RunEvent::new(
                        request.run_id,
                        RunEventKind::RetryScheduled,
                        RunEventPayload::Message {
                            summary: "model returned prose without a terminal tool call; reserved one submission turn"
                                .to_owned(),
                        },
                    )
                    .scoped(Some(request.image_id), Some(task.id.clone())),
                )
                .await?;
                continue;
            }

            let mut retry_requested = false;
            let mut terminal_reached = false;
            let mut feedback = Vec::new();
            for call in response_tool_calls {
                if terminal_reached || retry_requested {
                    let message = "tool call was not executed because an earlier call in the same assistant message selected a terminal or retry action";
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
                    self.append_model_message(
                        request,
                        task,
                        &mut messages,
                        ModelMessage {
                            role: ModelRole::Tool,
                            content: serde_json::json!({
                                "ok": false,
                                "error": {"code": "superseded_tool_call", "message": message}
                            })
                            .to_string(),
                            tool_call_id: Some(call.id.clone()),
                            tool_calls: Vec::new(),
                        },
                    )
                    .await?;
                    self.publish(
                        RunEvent::new(
                            request.run_id,
                            RunEventKind::ToolCallCompleted,
                            RunEventPayload::Tool {
                                call_id: call.id.clone(),
                                name: call.name.clone(),
                                summary: message.to_owned(),
                                success: false,
                            },
                        )
                        .scoped(Some(request.image_id), Some(task.id.clone())),
                    )
                    .await?;
                    continue;
                }
                if tool_calls_used >= self.config.max_tool_calls_per_task {
                    let message = format!(
                        "tool call budget exhausted ({}/{})",
                        tool_calls_used, self.config.max_tool_calls_per_task
                    );
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
                    self.append_model_message(
                        request,
                        task,
                        &mut messages,
                        ModelMessage {
                            role: ModelRole::Tool,
                            content: json!({
                                "ok": false,
                                "error": {
                                    "code": "tool_call_budget_exceeded",
                                    "message": message,
                                }
                            })
                            .to_string(),
                            tool_call_id: Some(call.id.clone()),
                            tool_calls: Vec::new(),
                        },
                    )
                    .await?;
                    self.publish(
                        RunEvent::new(
                            request.run_id,
                            RunEventKind::ToolCallCompleted,
                            RunEventPayload::Tool {
                                call_id: call.id,
                                name: call.name,
                                summary: message.clone(),
                                success: false,
                            },
                        )
                        .scoped(Some(request.image_id), Some(task.id.clone())),
                    )
                    .await?;
                    outcome
                        .issues
                        .push(runtime_issue("tool_call_budget_exceeded", &message));
                    terminal_reached = true;
                    continue;
                }
                tool_calls_used += 1;
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
                let signature = normalized_tool_signature(&call.name, &call.arguments);
                let (result, cache_hit) = if tools.is_read_only(&call.name) {
                    if let Some(cached) = deterministic_cache.get(&signature) {
                        (Ok(cached_tool_result(cached, &definitions)), true)
                    } else {
                        (
                            tools
                                .execute(&call.name, &context, call.arguments.clone())
                                .await,
                            false,
                        )
                    }
                } else {
                    (
                        tools
                            .execute(&call.name, &context, call.arguments.clone())
                            .await,
                        false,
                    )
                };
                match result {
                    Ok(result) => {
                        if tools.is_read_only(&call.name) && !cache_hit {
                            deterministic_cache.insert(signature, result.clone());
                        }
                        let artifacts = result.artifacts.clone();
                        known_artifacts |= !artifacts.is_empty();
                        let refined_artifacts = artifacts
                            .iter()
                            .filter(|artifact| artifact.role == ArtifactRole::RefinedCandidate)
                            .cloned()
                            .collect::<Vec<_>>();
                        for artifact in &artifacts {
                            self.store
                                .record_artifact(request.run_id, artifact)
                                .await
                                .map_err(RuntimeError::Store)?;
                            self.publish(
                                RunEvent::new(
                                    request.run_id,
                                    RunEventKind::ArtifactCreated,
                                    RunEventPayload::Artifact {
                                        artifact_ids: vec![artifact.id],
                                        summary: format!(
                                            "{} artifact from {} ({:?})",
                                            artifact.value.kind_name(),
                                            artifact.source_node,
                                            artifact.role
                                        ),
                                    },
                                )
                                .scoped(Some(request.image_id), Some(task.id.clone())),
                            )
                            .await?;
                        }
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
                                    summary: result.ui_summary.clone(),
                                    success: true,
                                },
                            )
                            .scoped(Some(request.image_id), Some(task.id.clone())),
                        )
                        .await?;
                        self.append_model_message(
                            request,
                            task,
                            &mut messages,
                            ModelMessage {
                                role: ModelRole::Tool,
                                content: serde_json::json!({
                                    "ok": true,
                                    "result": result.model_result,
                                })
                                .to_string(),
                                tool_call_id: Some(call.id.clone()),
                                tool_calls: Vec::new(),
                            },
                        )
                        .await?;
                        if !refined_artifacts.is_empty() && !cache_hit {
                            let decision = self
                                .process_artifacts(
                                    request,
                                    task,
                                    related,
                                    &refined_artifacts,
                                    retries,
                                )
                                .await?;
                            let state = if !decision.committed.is_empty() {
                                ArtifactValidationState::Valid
                            } else if !decision.review_queue.is_empty() {
                                ArtifactValidationState::NeedsReview
                            } else {
                                ArtifactValidationState::Invalid
                            };
                            for artifact in &refined_artifacts {
                                self.store
                                    .set_artifact_validation_state(
                                        request.run_id,
                                        artifact.id,
                                        state,
                                    )
                                    .await
                                    .map_err(RuntimeError::Store)?;
                            }
                            self.publish(
                                RunEvent::new(
                                    request.run_id,
                                    RunEventKind::ArtifactValidated,
                                    RunEventPayload::Artifact {
                                        artifact_ids: refined_artifacts
                                            .iter()
                                            .map(|artifact| artifact.id)
                                            .collect(),
                                        summary: format!(
                                            "deterministic validation result: {state:?}"
                                        ),
                                    },
                                )
                                .scoped(Some(request.image_id), Some(task.id.clone())),
                            )
                            .await?;
                            if !decision.committed.is_empty() {
                                self.publish(
                                    RunEvent::new(
                                        request.run_id,
                                        RunEventKind::ArtifactCommitted,
                                        RunEventPayload::Artifact {
                                            artifact_ids: refined_artifacts
                                                .iter()
                                                .map(|artifact| artifact.id)
                                                .collect(),
                                            summary: "validated artifact committed without geometry rewrite"
                                                .to_owned(),
                                        },
                                    )
                                    .scoped(Some(request.image_id), Some(task.id.clone())),
                                )
                                .await?;
                            }
                            let structured_issues = decision.issues.clone();
                            outcome.issues.extend(decision.issues);
                            outcome.committed.extend(decision.committed);
                            outcome.review_queue.extend(decision.review_queue);
                            if decision.retry {
                                feedback.push(
                                    serde_json::json!({
                                        "type": "artifact_validation_feedback",
                                        "artifact_references": refined_artifacts
                                            .iter()
                                            .map(VisionArtifact::reference)
                                            .collect::<Vec<_>>(),
                                        "issues": structured_issues,
                                    })
                                    .to_string(),
                                );
                                retry_requested = true;
                                retries += 1;
                            } else {
                                terminal_reached = true;
                            }
                        } else if call.name == "submit_annotation_candidates" {
                            match parse_candidates(
                                &call.arguments,
                                request.image_id,
                                task,
                                self.provider.name(),
                                &self.config.model,
                            ) {
                                Ok(candidates) => {
                                    if candidates.is_empty() {
                                        outcome.succeeded_empty = true;
                                    }
                                    let decision = self
                                        .process_candidates(
                                            request, task, related, candidates, retries, true,
                                        )
                                        .await?;
                                    let issue_summary = decision
                                        .issues
                                        .iter()
                                        .map(|issue| format!("{}: {}", issue.code, issue.message))
                                        .collect::<Vec<_>>()
                                        .join("; ");
                                    let structured_issues = decision.issues.clone();
                                    outcome.issues.extend(decision.issues);
                                    outcome.committed.extend(decision.committed);
                                    outcome.review_queue.extend(decision.review_queue);
                                    if decision.retry {
                                        feedback.push(
                                            serde_json::json!({
                                                "type": "validation_feedback",
                                                "summary": issue_summary,
                                                "issues": structured_issues,
                                            })
                                            .to_string(),
                                        );
                                        retry_requested = true;
                                        retries += 1;
                                    } else {
                                        terminal_reached = true;
                                    }
                                }
                                Err(error) => {
                                    let message = error.to_string();
                                    outcome
                                        .issues
                                        .push(runtime_issue("invalid_candidate", &message));
                                    feedback.push(format!(
                                        "Candidate rejected before validation: {message}. Correct the label/value shape and submit again."
                                    ));
                                    retry_requested = true;
                                    retries += 1;
                                }
                            }
                        } else if call.name == "request_human_review" {
                            outcome.issues.push(runtime_issue(
                                "model_requested_review",
                                "model explicitly requested human review",
                            ));
                            terminal_reached = true;
                        } else if call.name == "finish_task" {
                            outcome.succeeded_empty = true;
                            terminal_reached = true;
                        } else if matches!(
                            call.name.as_str(),
                            "accept_artifacts"
                                | "reject_artifacts"
                                | "request_artifact_refinement"
                                | "commit_artifacts"
                        ) {
                            match parse_artifact_action(&call.arguments) {
                                Ok(action) => {
                                    let mut selected =
                                        Vec::with_capacity(action.artifact_ids.len());
                                    for artifact_id in action.artifact_ids {
                                        let artifact = self
                                            .store
                                            .find_artifact(request.run_id, artifact_id)
                                            .await
                                            .map_err(RuntimeError::Store)?
                                            .ok_or_else(|| {
                                                RuntimeError::Candidate(format!(
                                                    "artifact {artifact_id} does not exist in run {}",
                                                    request.run_id
                                                ))
                                            })?;
                                        if artifact.image_id != request.image_id
                                            || artifact
                                                .task_id
                                                .as_ref()
                                                .is_some_and(|task_id| task_id != &task.id)
                                        {
                                            return Err(RuntimeError::Candidate(format!(
                                                "artifact {artifact_id} is outside the current image/task scope"
                                            )));
                                        }
                                        selected.push(artifact);
                                    }

                                    let artifact_ids = selected
                                        .iter()
                                        .map(|artifact| artifact.id)
                                        .collect::<Vec<_>>();
                                    match call.name.as_str() {
                                        "accept_artifacts" => {
                                            for artifact_id in &artifact_ids {
                                                self.store
                                                    .set_artifact_validation_state(
                                                        request.run_id,
                                                        *artifact_id,
                                                        ArtifactValidationState::Valid,
                                                    )
                                                    .await
                                                    .map_err(RuntimeError::Store)?;
                                            }
                                            self.publish(
                                                RunEvent::new(
                                                    request.run_id,
                                                    RunEventKind::ArtifactValidated,
                                                    RunEventPayload::Artifact {
                                                        artifact_ids: artifact_ids.clone(),
                                                        summary: "artifacts accepted for the current task"
                                                            .to_owned(),
                                                    },
                                                )
                                                .scoped(
                                                    Some(request.image_id),
                                                    Some(task.id.clone()),
                                                ),
                                            )
                                            .await?;
                                            feedback.push(
                                                json!({
                                                    "type": "artifact_action_result",
                                                    "action": "accepted",
                                                    "artifacts": selected
                                                        .iter()
                                                        .map(VisionArtifact::reference)
                                                        .collect::<Vec<_>>(),
                                                    "available_actions": [
                                                        "commit_artifacts",
                                                        "request_artifact_refinement",
                                                        "reject_artifacts"
                                                    ]
                                                })
                                                .to_string(),
                                            );
                                        }
                                        "reject_artifacts" => {
                                            for artifact_id in &artifact_ids {
                                                self.store
                                                    .set_artifact_validation_state(
                                                        request.run_id,
                                                        *artifact_id,
                                                        ArtifactValidationState::Invalid,
                                                    )
                                                    .await
                                                    .map_err(RuntimeError::Store)?;
                                            }
                                            self.publish(
                                                RunEvent::new(
                                                    request.run_id,
                                                    RunEventKind::ArtifactValidated,
                                                    RunEventPayload::Artifact {
                                                        artifact_ids,
                                                        summary: action.reason.unwrap_or_else(
                                                            || {
                                                                "artifacts rejected by the model"
                                                                    .to_owned()
                                                            },
                                                        ),
                                                    },
                                                )
                                                .scoped(
                                                    Some(request.image_id),
                                                    Some(task.id.clone()),
                                                ),
                                            )
                                            .await?;
                                            feedback.push(
                                                json!({
                                                    "type": "artifact_action_result",
                                                    "action": "rejected",
                                                    "available_actions": tools
                                                        .definitions_for_task(&task.id)
                                                        .into_iter()
                                                        .map(|definition| definition.name)
                                                        .collect::<Vec<_>>(),
                                                })
                                                .to_string(),
                                            );
                                        }
                                        "request_artifact_refinement" => {
                                            feedback.push(
                                                json!({
                                                    "type": "artifact_refinement_requested",
                                                    "artifacts": selected
                                                        .iter()
                                                        .map(VisionArtifact::reference)
                                                        .collect::<Vec<_>>(),
                                                    "reason": action.reason,
                                                    "instruction": "Call a registered refinement or evidence tool, then inspect the returned artifact references."
                                                })
                                                .to_string(),
                                            );
                                        }
                                        "commit_artifacts" => {
                                            let decision = self
                                                .process_artifacts(
                                                    request, task, related, &selected, retries,
                                                )
                                                .await?;
                                            let state = if !decision.committed.is_empty() {
                                                ArtifactValidationState::Valid
                                            } else if !decision.review_queue.is_empty() {
                                                ArtifactValidationState::NeedsReview
                                            } else {
                                                ArtifactValidationState::Invalid
                                            };
                                            for artifact_id in &artifact_ids {
                                                self.store
                                                    .set_artifact_validation_state(
                                                        request.run_id,
                                                        *artifact_id,
                                                        state,
                                                    )
                                                    .await
                                                    .map_err(RuntimeError::Store)?;
                                            }
                                            self.publish(
                                                RunEvent::new(
                                                    request.run_id,
                                                    RunEventKind::ArtifactValidated,
                                                    RunEventPayload::Artifact {
                                                        artifact_ids: artifact_ids.clone(),
                                                        summary: format!(
                                                            "artifact commit validation result: {state:?}"
                                                        ),
                                                    },
                                                )
                                                .scoped(
                                                    Some(request.image_id),
                                                    Some(task.id.clone()),
                                                ),
                                            )
                                            .await?;
                                            if !decision.committed.is_empty() {
                                                self.publish(
                                                    RunEvent::new(
                                                        request.run_id,
                                                        RunEventKind::ArtifactCommitted,
                                                        RunEventPayload::Artifact {
                                                            artifact_ids,
                                                            summary: "accepted artifacts committed"
                                                                .to_owned(),
                                                        },
                                                    )
                                                    .scoped(
                                                        Some(request.image_id),
                                                        Some(task.id.clone()),
                                                    ),
                                                )
                                                .await?;
                                            }
                                            let structured_issues = decision.issues.clone();
                                            outcome.issues.extend(decision.issues);
                                            outcome.committed.extend(decision.committed);
                                            outcome.review_queue.extend(decision.review_queue);
                                            if decision.retry {
                                                feedback.push(
                                                    json!({
                                                        "type": "artifact_validation_feedback",
                                                        "artifact_references": selected
                                                            .iter()
                                                            .map(VisionArtifact::reference)
                                                            .collect::<Vec<_>>(),
                                                        "issues": structured_issues,
                                                    })
                                                    .to_string(),
                                                );
                                                retry_requested = true;
                                                retries += 1;
                                            } else {
                                                terminal_reached = true;
                                            }
                                        }
                                        _ => unreachable!(),
                                    }
                                }
                                Err(message) => {
                                    outcome
                                        .issues
                                        .push(runtime_issue("invalid_artifact_action", &message));
                                    feedback.push(
                                        json!({
                                            "type": "artifact_action_error",
                                            "message": message,
                                            "available_actions": tools
                                                .definitions_for_task(&task.id)
                                                .into_iter()
                                                .map(|definition| definition.name)
                                                .collect::<Vec<_>>(),
                                        })
                                        .to_string(),
                                    );
                                    retry_requested = true;
                                    retries += 1;
                                }
                            }
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
                        self.append_model_message(
                            request,
                            task,
                            &mut messages,
                            ModelMessage {
                                role: ModelRole::Tool,
                                content: serde_json::json!({
                                    "ok": false,
                                    "error": {"code": "tool_execution_failed", "message": message}
                                })
                                .to_string(),
                                tool_call_id: Some(call.id),
                                tool_calls: Vec::new(),
                            },
                        )
                        .await?;
                        if call.name == "submit_annotation_candidates" {
                            outcome
                                .issues
                                .push(runtime_issue("invalid_candidate", &message));
                            feedback.push(format!(
                                "Candidate tool call was rejected: {message}. Correct it and retry."
                            ));
                            retry_requested = true;
                            retries += 1;
                        }
                    }
                }
            }
            if retry_requested {
                recovery_turns_used += 1;
                if recovery_turns_used > self.config.max_recovery_turns_per_task {
                    outcome.issues.push(runtime_issue(
                        "recovery_turn_budget_exceeded",
                        "validator/tool recovery budget was exhausted",
                    ));
                    feedback.push(
                        json!({
                            "type": "recovery_budget_exhausted",
                            "available_actions": ["request_human_review", "finish_task"]
                        })
                        .to_string(),
                    );
                    retry_requested = false;
                    terminal_reached = true;
                }
            }
            validate_model_message_history(&messages)?;
            for content in feedback {
                self.append_model_message(
                    request,
                    task,
                    &mut messages,
                    ModelMessage {
                        role: ModelRole::User,
                        content,
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                    },
                )
                .await?;
            }
            if terminal_reached {
                return Ok(outcome);
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
        if outcome.committed.is_empty()
            && outcome.review_queue.is_empty()
            && !outcome.succeeded_empty
        {
            outcome.issues.push(runtime_issue(
                "max_steps_or_no_submission",
                "task ended without a committed candidate",
            ));
        }
        Ok(outcome)
    }

    async fn process_artifacts(
        &self,
        request: &ImageRunRequest,
        task: &TaskConfig,
        related: &[Annotation],
        artifacts: &[VisionArtifact],
        retries: u32,
    ) -> Result<CandidateDecision, RuntimeError> {
        let mut candidates = Vec::with_capacity(artifacts.len());
        for artifact in artifacts {
            if artifact.image_id != request.image_id {
                return Err(RuntimeError::Candidate(format!(
                    "artifact {} belongs to a different image",
                    artifact.id
                )));
            }
            if artifact.task_id.as_ref().is_some_and(|id| id != &task.id) {
                return Err(RuntimeError::Candidate(format!(
                    "artifact {} belongs to task {:?}, not {:?}",
                    artifact.id,
                    artifact.task_id.as_ref().map(TaskId::as_str),
                    task.id.as_str()
                )));
            }
            let source = if artifact.provenance.model.is_some() {
                AnnotationSource::ModelAndTool
            } else {
                AnnotationSource::DeterministicTool
            };
            let annotation = Annotation {
                id: AnnotationId::new(),
                image_id: artifact.image_id,
                task_id: task.id.clone(),
                label: artifact.label.clone(),
                value: artifact.value.as_annotation_value(),
                attributes: BTreeMap::new(),
                confidence: artifact.confidence,
                source,
                review_status: ReviewStatus::Draft,
                provenance: AnnotationProvenance {
                    provider: artifact.provenance.provider.clone(),
                    model: artifact.provenance.model.clone(),
                    tool_names: artifact.provenance.tool.clone().into_iter().collect(),
                    artifact_ids: vec![artifact.id],
                    ..AnnotationProvenance::default()
                },
                created_at: Utc::now(),
            };
            annotation
                .validate()
                .map_err(|error| RuntimeError::Candidate(error.to_string()))?;
            candidates.push(annotation);
        }
        self.process_candidates(request, task, related, candidates, retries, false)
            .await
    }

    async fn process_candidates(
        &self,
        request: &ImageRunRequest,
        task: &TaskConfig,
        related: &[Annotation],
        candidates: Vec<Annotation>,
        retries: u32,
        apply_refiners: bool,
    ) -> Result<CandidateDecision, RuntimeError> {
        let validators = self.skill.validators();
        let refiners = self.skill.refiners();
        let mut output = CandidateDecision::default();
        let peer_candidates = candidates.clone();
        for mut candidate in candidates {
            let mut latest_refiner_artifact = None;
            let mut artifact_revision = 0_u32;
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
            for refiner_id in task.refiners.iter().filter(|_| apply_refiners) {
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
                if latest_refiner_artifact.is_none() {
                    let original = VisionArtifact {
                        id: ArtifactId::new(),
                        image_id: candidate.image_id,
                        task_id: Some(task.id.clone()),
                        label: candidate.label.clone(),
                        role: ArtifactRole::Candidate,
                        value: VisionArtifactValue::from_annotation_value(&before.value),
                        source_node: format!("{refiner_id}.input"),
                        confidence: before.confidence,
                        metadata: BTreeMap::new(),
                        validation_state: ArtifactValidationState::Unvalidated,
                        provenance: ArtifactProvenance {
                            provider: candidate.provenance.provider.clone(),
                            model: candidate.provenance.model.clone(),
                            ..ArtifactProvenance::default()
                        },
                        revision: 1,
                        replaces_artifact_id: None,
                        created_at: Utc::now(),
                    };
                    self.record_artifact_created(request, task, &original, "original candidate")
                        .await?;
                    latest_refiner_artifact = Some(original.id);
                    artifact_revision = 1;
                }
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
                artifact_revision = artifact_revision.saturating_add(1);
                let refined_artifact = VisionArtifact {
                    id: ArtifactId::new(),
                    image_id: candidate.image_id,
                    task_id: Some(task.id.clone()),
                    label: candidate.label.clone(),
                    role: ArtifactRole::RefinedCandidate,
                    value: VisionArtifactValue::from_annotation_value(&candidate.value),
                    source_node: refiner_id.clone(),
                    confidence: Some(result.confidence),
                    metadata: BTreeMap::from([(
                        "refiner_summary".to_owned(),
                        json!(result.summary),
                    )]),
                    validation_state: ArtifactValidationState::Unvalidated,
                    provenance: ArtifactProvenance {
                        provider: candidate.provenance.provider.clone(),
                        model: candidate.provenance.model.clone(),
                        tool: Some(refiner_id.clone()),
                        input_artifact_ids: latest_refiner_artifact.into_iter().collect(),
                        ..ArtifactProvenance::default()
                    },
                    revision: artifact_revision,
                    replaces_artifact_id: latest_refiner_artifact,
                    created_at: Utc::now(),
                };
                refined_artifact
                    .validate()
                    .map_err(|error| RuntimeError::Candidate(error.to_string()))?;
                self.record_artifact_created(request, task, &refined_artifact, "refined candidate")
                    .await?;
                latest_refiner_artifact = Some(refined_artifact.id);
                candidate.provenance.artifact_ids.push(refined_artifact.id);
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
                    max_retries: self.config.max_retries,
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
                    self.finalize_refiner_artifact(
                        request,
                        task,
                        latest_refiner_artifact,
                        ArtifactValidationState::Valid,
                        true,
                    )
                    .await?;
                    output.committed.push(candidate);
                }
                ReviewDecision::Retry { reasons } => {
                    self.finalize_refiner_artifact(
                        request,
                        task,
                        latest_refiner_artifact,
                        ArtifactValidationState::Invalid,
                        false,
                    )
                    .await?;
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
                    self.finalize_refiner_artifact(
                        request,
                        task,
                        latest_refiner_artifact,
                        ArtifactValidationState::NeedsReview,
                        false,
                    )
                    .await?;
                    output.review_queue.push(candidate);
                }
                ReviewDecision::Reject { reasons } => {
                    self.finalize_refiner_artifact(
                        request,
                        task,
                        latest_refiner_artifact,
                        ArtifactValidationState::Invalid,
                        false,
                    )
                    .await?;
                    output.issues.push(runtime_issue(
                        "policy_rejected",
                        &format!("candidate rejected: {}", reasons.join("; ")),
                    ));
                }
            }
        }
        Ok(output)
    }

    async fn record_artifact_created(
        &self,
        request: &ImageRunRequest,
        task: &TaskConfig,
        artifact: &VisionArtifact,
        stage: &str,
    ) -> Result<(), RuntimeError> {
        artifact
            .validate()
            .map_err(|error| RuntimeError::Candidate(error.to_string()))?;
        self.store
            .record_artifact(request.run_id, artifact)
            .await
            .map_err(RuntimeError::Store)?;
        self.publish(
            RunEvent::new(
                request.run_id,
                RunEventKind::ArtifactCreated,
                RunEventPayload::Artifact {
                    artifact_ids: vec![artifact.id],
                    summary: format!(
                        "{stage}: {} revision {} from {}",
                        artifact.value.kind_name(),
                        artifact.revision,
                        artifact.source_node
                    ),
                },
            )
            .scoped(Some(request.image_id), Some(task.id.clone())),
        )
        .await
    }

    async fn finalize_refiner_artifact(
        &self,
        request: &ImageRunRequest,
        task: &TaskConfig,
        artifact_id: Option<ArtifactId>,
        state: ArtifactValidationState,
        committed: bool,
    ) -> Result<(), RuntimeError> {
        let Some(artifact_id) = artifact_id else {
            return Ok(());
        };
        self.store
            .set_artifact_validation_state(request.run_id, artifact_id, state)
            .await
            .map_err(RuntimeError::Store)?;
        self.publish(
            RunEvent::new(
                request.run_id,
                RunEventKind::ArtifactValidated,
                RunEventPayload::Artifact {
                    artifact_ids: vec![artifact_id],
                    summary: format!("refined artifact validation result: {state:?}"),
                },
            )
            .scoped(Some(request.image_id), Some(task.id.clone())),
        )
        .await?;
        if committed {
            self.publish(
                RunEvent::new(
                    request.run_id,
                    RunEventKind::ArtifactCommitted,
                    RunEventPayload::Artifact {
                        artifact_ids: vec![artifact_id],
                        summary: "validated refined artifact committed without geometry rewrite"
                            .to_owned(),
                    },
                )
                .scoped(Some(request.image_id), Some(task.id.clone())),
            )
            .await?;
        }
        Ok(())
    }

    async fn publish(&self, event: RunEvent) -> Result<(), RuntimeError> {
        self.store
            .record_event(&event)
            .await
            .map_err(RuntimeError::Store)?;
        self.event_bus.send(event);
        Ok(())
    }

    async fn append_model_message(
        &self,
        request: &ImageRunRequest,
        task: &TaskConfig,
        messages: &mut Vec<ModelMessage>,
        message: ModelMessage,
    ) -> Result<(), RuntimeError> {
        self.store
            .record_model_message(
                request.run_id,
                Some(request.image_id),
                Some(&task.id),
                &message,
            )
            .await
            .map_err(RuntimeError::Store)?;
        messages.push(message);
        Ok(())
    }

    async fn complete_task(
        &self,
        request: &ImageRunRequest,
        task: &TaskConfig,
        status: TaskRunStatus,
        reason: Option<&str>,
    ) -> Result<(), RuntimeError> {
        self.store
            .set_task_run_status(request.run_id, request.image_id, &task.id, status, reason)
            .await
            .map_err(RuntimeError::Store)?;
        self.publish(
            RunEvent::new(
                request.run_id,
                RunEventKind::TaskCompleted,
                RunEventPayload::Message {
                    summary: format!(
                        "task {} finished with status {}{}",
                        task.id,
                        serde_json::to_value(status)
                            .ok()
                            .and_then(|value| value.as_str().map(str::to_owned))
                            .unwrap_or_else(|| "unknown".to_owned()),
                        reason.map_or_else(String::new, |reason| format!(": {reason}")),
                    ),
                },
            )
            .scoped(Some(request.image_id), Some(task.id.clone())),
        )
        .await
    }

    async fn publish_task_failure(
        &self,
        request: &ImageRunRequest,
        task: &TaskConfig,
        elapsed_ms: u64,
        error_code: &str,
        summary: &str,
    ) -> Result<(), RuntimeError> {
        self.publish(
            RunEvent::new(
                request.run_id,
                RunEventKind::TaskFailed,
                RunEventPayload::TaskFailure {
                    task_id: task.id.clone(),
                    node_id: task.id.to_string(),
                    elapsed_ms,
                    error_code: error_code.to_owned(),
                    summary: summary.to_owned(),
                },
            )
            .scoped(Some(request.image_id), Some(task.id.clone())),
        )
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
            RunStatus::Completed | RunStatus::CompletedWithReview | RunStatus::Partial => {
                RunEventKind::RunCompleted
            }
            RunStatus::Cancelled => RunEventKind::RunCancelled,
            RunStatus::BudgetExceeded => RunEventKind::RunBudgetExceeded,
            RunStatus::Interrupted => RunEventKind::RunInterrupted,
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
    succeeded_empty: bool,
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

fn available_definitions(
    definitions: &[ToolDefinition],
    known_artifacts: bool,
) -> Vec<ToolDefinition> {
    if !known_artifacts {
        return definitions.to_vec();
    }
    definitions
        .iter()
        .filter(|definition| {
            definition.read_only
                || matches!(
                    definition.name.as_str(),
                    "accept_artifacts"
                        | "reject_artifacts"
                        | "request_artifact_refinement"
                        | "commit_artifacts"
                        | "request_human_review"
                        | "finish_task"
                )
        })
        .cloned()
        .collect()
}

fn cached_tool_result(result: &ToolResult, definitions: &[ToolDefinition]) -> ToolResult {
    let available_actions = available_definitions(definitions, !result.artifacts.is_empty())
        .into_iter()
        .map(|definition| definition.name)
        .collect::<Vec<_>>();
    ToolResult {
        persisted_result: json!({
            "cached": true,
            "original_result": result.persisted_result,
        }),
        model_result: json!({
            "cached": true,
            "message": "identical deterministic tool call reused its existing result; no inference was executed",
            "result": result.model_result,
            "artifact_references": result.artifacts.iter().map(VisionArtifact::reference).collect::<Vec<_>>(),
            "available_actions": available_actions,
        }),
        ui_summary: format!("cache hit · {}", result.ui_summary),
        artifacts: result.artifacts.clone(),
    }
}

fn is_operational_issue(code: &str) -> bool {
    matches!(
        code,
        "dependency_failed"
            | "invalid_candidate"
            | "max_steps_or_no_submission"
            | "model_requested_review"
            | "repeated_tool_call"
            | "task_timeout"
            | "task_runtime_failed"
            | "tool_call_budget_exceeded"
    )
}

fn validate_model_message_history(messages: &[ModelMessage]) -> Result<(), RuntimeError> {
    let mut pending = VecDeque::<annotagent_core::ToolCallId>::new();
    let mut seen = BTreeSet::new();
    for (index, message) in messages.iter().enumerate() {
        match message.role {
            ModelRole::Assistant if !message.tool_calls.is_empty() => {
                if !pending.is_empty() {
                    return Err(RuntimeError::Protocol(format!(
                        "assistant message {index} arrived before all prior tool results"
                    )));
                }
                for call in &message.tool_calls {
                    if !seen.insert(call.id.clone()) {
                        return Err(RuntimeError::Protocol(format!(
                            "duplicate tool_call_id {:?}",
                            call.id.as_str()
                        )));
                    }
                    pending.push_back(call.id.clone());
                }
            }
            ModelRole::Tool => {
                let expected = pending.pop_front().ok_or_else(|| {
                    RuntimeError::Protocol(format!(
                        "tool message {index} has no preceding assistant tool call"
                    ))
                })?;
                let actual = message.tool_call_id.as_ref().ok_or_else(|| {
                    RuntimeError::Protocol(format!("tool message {index} is missing tool_call_id"))
                })?;
                if actual != &expected {
                    return Err(RuntimeError::Protocol(format!(
                        "tool message {index} answered {:?}, expected {:?}",
                        actual.as_str(),
                        expected.as_str()
                    )));
                }
                if !message.tool_calls.is_empty() {
                    return Err(RuntimeError::Protocol(format!(
                        "tool message {index} cannot declare nested tool calls"
                    )));
                }
            }
            _ => {
                if !pending.is_empty() {
                    return Err(RuntimeError::Protocol(format!(
                        "message {index} interrupted {} pending tool result(s)",
                        pending.len()
                    )));
                }
                if !message.tool_calls.is_empty() {
                    return Err(RuntimeError::Protocol(format!(
                        "only assistant messages may contain tool calls (message {index})"
                    )));
                }
            }
        }
    }
    if pending.is_empty() {
        Ok(())
    } else {
        Err(RuntimeError::Protocol(format!(
            "history is missing {} tool result message(s)",
            pending.len()
        )))
    }
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
            "accept_artifacts",
            "Mark existing typed artifacts as accepted. This does not commit annotations; call commit_artifacts when the accepted geometry is final.",
            artifact_action_schema(false),
        )),
        Arc::new(EchoTool::new(
            "reject_artifacts",
            "Reject existing typed artifacts while preserving their trace and provenance.",
            artifact_action_schema(true),
        )),
        Arc::new(EchoTool::new(
            "request_artifact_refinement",
            "Request another registered refinement/evidence step for existing artifacts. The runtime keeps all geometry typed and referenced by artifact_id.",
            artifact_action_schema(true),
        )),
        Arc::new(EchoTool::new(
            "commit_artifacts",
            "Validate and commit existing typed artifacts by artifact_id. Never copy their coordinates into submit_annotation_candidates.",
            artifact_action_schema(false),
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

fn artifact_action_schema(reason_required: bool) -> Value {
    let required = if reason_required {
        json!(["artifact_ids", "reason"])
    } else {
        json!(["artifact_ids"])
    };
    json!({
        "type": "object",
        "properties": {
            "artifact_ids": {
                "type": "array",
                "items": {"type": "string"},
                "minItems": 1
            },
            "reason": {"type": "string"}
        },
        "required": required,
        "additionalProperties": false
    })
}

#[derive(Debug, Deserialize)]
struct ArtifactActionArguments {
    artifact_ids: Vec<ArtifactId>,
    #[serde(default)]
    reason: Option<String>,
}

fn parse_artifact_action(arguments: &Value) -> Result<ArtifactActionArguments, String> {
    serde_json::from_value(arguments.clone())
        .map_err(|error| format!("invalid artifact action arguments: {error}"))
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
        Ok(ToolResult::structured(
            format!(
                "{} request accepted for runtime processing",
                self.definition.name
            ),
            arguments,
        ))
    }
}

#[cfg(test)]
mod tests {
    use annotagent_core::{ModelToolCall, TaskId, ToolCallId};

    use super::*;

    #[test]
    fn candidate_parser_rejects_wrong_task_kind() {
        let task = TaskConfig {
            id: TaskId::from("scene"),
            display_name: None,
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

    #[test]
    fn tool_history_requires_one_ordered_result_per_call() {
        let first = ToolCallId::new("first");
        let second = ToolCallId::new("second");
        let assistant = ModelMessage {
            role: ModelRole::Assistant,
            content: String::new(),
            tool_call_id: None,
            tool_calls: vec![
                ModelToolCall {
                    id: first.clone(),
                    name: "detect".to_owned(),
                    arguments: json!({}),
                },
                ModelToolCall {
                    id: second.clone(),
                    name: "segment".to_owned(),
                    arguments: json!({}),
                },
            ],
        };
        let tool = |id| ModelMessage {
            role: ModelRole::Tool,
            content: json!({"ok": true, "result": {}}).to_string(),
            tool_call_id: Some(id),
            tool_calls: Vec::new(),
        };
        assert!(
            validate_model_message_history(&[
                assistant.clone(),
                tool(first.clone()),
                tool(second.clone())
            ])
            .is_ok()
        );
        assert!(
            validate_model_message_history(&[assistant.clone(), tool(second), tool(first)])
                .is_err()
        );
        assert!(validate_model_message_history(&[assistant]).is_err());
    }
}
