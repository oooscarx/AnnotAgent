use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use annotagent_application::{
    AnnotAgentApplication, DetectionWorkerSettings, LocalApplication, Settings, load_settings,
    stable_project_id,
};
use annotagent_core::{
    AgentSessionStatus, BindingMutationActor, InputModality, ModelBindingId, ModelBindingMatch,
    ModelBindingRole, ModelCapability, ModelProfileId, ModelProfileStatus,
    PipelineBuilderConstraints, ProjectId, ProjectModelBinding, ProjectSchema,
    ProviderHealthStatus, ProviderId, RunEvent, RunEventPayload, RunStatus,
};
use annotagent_provider::HttpVisionWorkerClient;
use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::runner;

mod theme;

use theme::{AnnotAgentTheme, StatusTone};

struct ActiveRun {
    run_id: annotagent_core::RunId,
    events: broadcast::Receiver<RunEvent>,
}

#[derive(Debug, Clone)]
struct ProjectContext {
    id: String,
    stable_id: ProjectId,
    name: String,
    workflow: String,
    skills: String,
}

impl ProjectContext {
    fn load(path: &Path) -> Result<Self> {
        let yaml = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read Project {}", path.display()))?;
        let project = ProjectSchema::from_yaml(&yaml).map_err(|error| anyhow::anyhow!(error))?;
        let skills = project
            .project
            .enabled_skill_versions()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        Ok(Self {
            id: path
                .parent()
                .and_then(Path::file_name)
                .and_then(std::ffi::OsStr::to_str)
                .context("Project directory has no usable id")?
                .to_owned(),
            stable_id: stable_project_id(path.parent().unwrap_or(path)),
            name: project.project.name,
            workflow: format!("Configured task graph@v{}", project.version),
            skills: if skills.is_empty() {
                "None".to_owned()
            } else {
                skills.join(", ")
            },
        })
    }
}

struct TuiState {
    project: Option<PathBuf>,
    project_context: Option<ProjectContext>,
    application: Arc<LocalApplication>,
    input: String,
    trace: Vec<String>,
    status: RunStatus,
    current_task: String,
    usage: String,
    active: Option<ActiveRun>,
    model_lines: Vec<String>,
    quit: bool,
}

impl TuiState {
    fn new(project: Option<PathBuf>, application: Arc<LocalApplication>) -> Result<Self> {
        let project_context = project.as_deref().map(ProjectContext::load).transpose()?;
        let trace = if project.is_some() {
            "Ready. Press r or enter /run to start the deterministic demo."
        } else {
            "No project opened. Use /open <project.yaml> or /init for setup help."
        };
        let model_lines = registry_model_lines(application.as_ref())?
            .into_iter()
            .chain(model_lines(&workspace_settings(application.as_ref())?))
            .collect();
        Ok(Self {
            project,
            project_context,
            application,
            input: String::new(),
            trace: vec![trace.to_owned()],
            status: RunStatus::Pending,
            current_task: "-".to_owned(),
            usage: "input 0 · output 0 · cost 0".to_owned(),
            active: None,
            model_lines,
            quit: false,
        })
    }

    fn push(&mut self, message: impl Into<String>) {
        self.trace.push(message.into());
        if self.trace.len() > 300 {
            self.trace.drain(..100);
        }
    }

    fn start(&mut self) -> Result<()> {
        if self.active.is_some() {
            self.push("A run is already active.");
            return Ok(());
        }
        let Some(project) = self.project.as_deref() else {
            self.push("No project opened. Use /open <project.yaml> first.");
            return Ok(());
        };
        let mut events = self.application.subscribe();
        while events.try_recv().is_ok() {}
        let started = self
            .application
            .start_run_path(project, "openai_compatible", None)?;
        self.active = Some(ActiveRun {
            run_id: started.run_id,
            events,
        });
        self.push(format!("Started image {}", started.image_path.display()));
        Ok(())
    }

    fn drain_events(&mut self) {
        let mut received = Vec::new();
        if let Some(active) = &mut self.active {
            loop {
                match active.events.try_recv() {
                    Ok(event) => received.push(event),
                    Err(
                        broadcast::error::TryRecvError::Empty
                        | broadcast::error::TryRecvError::Closed,
                    ) => break,
                    Err(broadcast::error::TryRecvError::Lagged(count)) => {
                        self.trace.push(format!("event view lagged by {count}"));
                    }
                }
            }
        }
        for event in received {
            self.apply_event(&event);
        }
    }

    fn apply_event(&mut self, event: &RunEvent) {
        if let Some(task) = &event.task_id {
            self.current_task = task.to_string();
        }
        match &event.payload {
            RunEventPayload::State { to, reason, .. } => {
                self.status = *to;
                self.push(format!(
                    "{:?}: {}",
                    event.kind,
                    reason.as_deref().unwrap_or("")
                ));
            }
            RunEventPayload::Usage { totals } => {
                self.usage = format!(
                    "input {} · output {} · requests {} · cost {}",
                    totals.input_tokens, totals.output_tokens, totals.requests, totals.cost
                );
            }
            RunEventPayload::Tool { name, summary, .. } => {
                self.push(format!("tool {name}: {summary}"));
            }
            RunEventPayload::Validation { issue_codes, .. } => {
                self.push(format!("validation: {}", issue_codes.join(", ")));
            }
            RunEventPayload::Annotation { summary, .. }
            | RunEventPayload::Artifact { summary, .. }
            | RunEventPayload::ProviderFailure { summary, .. }
            | RunEventPayload::TaskFailure { summary, .. }
            | RunEventPayload::Message { summary } => {
                self.push(format!("{:?}: {summary}", event.kind));
            }
            RunEventPayload::Progress {
                current_step,
                max_steps,
                ..
            } => self.push(format!("model step {current_step}/{max_steps}")),
        }
    }

    async fn collect_finished(&mut self) {
        if matches!(
            self.status,
            RunStatus::CompletedWithReview
                | RunStatus::Completed
                | RunStatus::Partial
                | RunStatus::Cancelled
                | RunStatus::BudgetExceeded
                | RunStatus::Failed
                | RunStatus::Interrupted
        ) && let Some(active) = self.active.take()
        {
            match self.application.wait_run(active.run_id).await {
                Ok(result) => {
                    self.status = result.status;
                    self.push(format!(
                        "finished: committed {}, review {}, issues {}",
                        result.committed.len(),
                        result.review_queue.len(),
                        result.issues.len()
                    ));
                }
                Err(error) => self.push(format!("run failed: {error:#}")),
            }
        }
    }

    async fn pause_or_resume(&mut self) {
        let Some(active) = &self.active else {
            self.push("No active run.");
            return;
        };
        let run_id = active.run_id;
        let result = if self.status == RunStatus::Paused {
            self.application
                .resume_run(run_id)
                .await
                .map(|()| "resumed")
        } else {
            self.application.pause_run(run_id).await.map(|()| "paused")
        };
        match result {
            Ok(message) => self.push(message),
            Err(error) => self.push(format!("control error: {error}")),
        }
    }

    async fn cancel(&mut self) {
        let Some(active) = &self.active else {
            self.push("No active run.");
            return;
        };
        let run_id = active.run_id;
        match self.application.cancel_run(run_id).await {
            Ok(()) => self.push("cancellation requested"),
            Err(error) => self.push(format!("control error: {error}")),
        }
    }

    fn inspect_latest(&mut self) -> Result<()> {
        let run_id = self
            .active
            .as_ref()
            .map(|active| active.run_id)
            .or_else(|| {
                self.application
                    .store()
                    .list_runs()
                    .ok()
                    .and_then(|runs| runs.into_iter().next().map(|run| run.id))
            });
        let Some(run_id) = run_id else {
            self.push("No Run history is available to inspect.");
            return Ok(());
        };
        let history = self.application.store().history(run_id)?;
        let snapshot = history
            .run
            .workflow_snapshot_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok());
        let workflow = snapshot
            .as_ref()
            .and_then(|value| value.pointer("/selected_workflow"))
            .map_or_else(
                || "Configured task graph".to_owned(),
                |value| {
                    format!(
                        "{}@v{}",
                        value["workflow_id"].as_str().unwrap_or("configured"),
                        value["version"].as_u64().unwrap_or(1)
                    )
                },
            );
        let checkpoint = snapshot
            .as_ref()
            .and_then(|value| value.pointer("/checkpoint"));
        let fallback = checkpoint
            .and_then(|value| value["activated_fallbacks"].as_array())
            .map(|nodes| {
                nodes
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|nodes| !nodes.is_empty())
            .unwrap_or_else(|| "none".to_owned());
        let current = history.task_runs.last();
        let issue_codes = history
            .events
            .iter()
            .filter_map(|event| match &event.payload {
                RunEventPayload::Validation { issue_codes, .. } => Some(issue_codes.as_slice()),
                _ => None,
            })
            .flatten()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        let retries = history.usage.iter().fold(0_u32, |total, usage| {
            total.saturating_add(usage.retry_count)
        });
        let timed_out = history.events.iter().any(|event| match &event.payload {
            RunEventPayload::ProviderFailure { error_code, .. }
            | RunEventPayload::TaskFailure { error_code, .. } => {
                error_code.to_ascii_lowercase().contains("timeout")
            }
            _ => false,
        });
        let review_suspended = history.run.status == RunStatus::AwaitingReview
            || history
                .task_runs
                .iter()
                .any(|task| task.status == annotagent_core::TaskRunStatus::NeedsReview);
        let lines = vec![
            format!(
                "inspect {} · Project {} · Workflow {} · status {:?}",
                run_id, history.run.project_name, workflow, history.run.status
            ),
            format!(
                "node {} · status {} · Artifacts {} · issues {}",
                current.map_or_else(|| "none".to_owned(), |task| task.task_id.to_string()),
                current.map_or_else(
                    || "not started".to_owned(),
                    |task| format!("{:?}", task.status).to_ascii_lowercase()
                ),
                history.artifacts.len(),
                if issue_codes.is_empty() {
                    "none".to_owned()
                } else {
                    issue_codes.into_iter().collect::<Vec<_>>().join(", ")
                }
            ),
            format!(
                "model {}/{} · retries {} · fallback {} · timeout {}",
                history.run.provider, history.run.model, retries, fallback, timed_out
            ),
            format!(
                "checkpoint {} · review suspension {}",
                checkpoint.is_some(),
                review_suspended
            ),
        ];
        for line in lines {
            self.push(line);
        }
        Ok(())
    }

    fn latest_run_id(&self) -> Option<annotagent_core::RunId> {
        self.active
            .as_ref()
            .map(|active| active.run_id)
            .or_else(|| {
                self.application
                    .store()
                    .list_runs()
                    .ok()
                    .and_then(|runs| runs.into_iter().next().map(|run| run.id))
            })
    }

    fn list_artifacts(&mut self) -> Result<()> {
        let run_id = self
            .latest_run_id()
            .context("No Run history is available.")?;
        let inspection = self.application.inspect_run_pipeline_artifacts(run_id)?;
        self.push(format!("Artifacts for Run {run_id}"));
        for node in inspection.nodes {
            let kinds = node
                .outputs
                .iter()
                .map(|artifact| {
                    format!(
                        "{:?}",
                        annotagent_core::PipelineArtifact::artifact_type(artifact)
                    )
                    .to_ascii_lowercase()
                })
                .collect::<Vec<_>>()
                .join(", ");
            self.push(format!(
                "{} · {:?} · {} · cache {}",
                node.node_id,
                node.status,
                if kinds.is_empty() {
                    "no output"
                } else {
                    &kinds
                },
                if node.cache_hit { "hit" } else { "miss" }
            ));
        }
        Ok(())
    }

    async fn replay(&mut self, requested_node: Option<&str>) -> Result<()> {
        let run_id = self
            .latest_run_id()
            .context("No Run history is available.")?;
        let inspection = self.application.inspect_run_pipeline_artifacts(run_id)?;
        let node_id = requested_node
            .map(str::to_owned)
            .or_else(|| inspection.nodes.last().map(|node| node.node_id.clone()))
            .context("This Run has no replayable node.")?;
        self.push(format!("Sandbox Replay started from {node_id}"));
        let report = self
            .application
            .replay_run_from_node(
                run_id,
                &node_id,
                &workspace_settings(self.application.as_ref())?,
            )
            .await?;
        self.push(format!(
            "Replay completed · preserved {} · re-executed {}",
            report.preserved_upstream_nodes.join(", "),
            report.reexecuted_nodes.join(", ")
        ));
        Ok(())
    }

    fn providers(&mut self, action: Option<&str>, id: Option<&str>) -> Result<()> {
        let providers = self.application.store().list_provider_profiles()?;
        match action {
            None => {
                if providers.is_empty() {
                    self.push(
                        "No Provider Profiles. Use the GUI to add credentials safely, or configure an environment-variable reference.",
                    );
                }
                for provider in providers {
                    let model_count = self
                        .application
                        .store()
                        .list_model_profiles(Some(provider.id), false)?
                        .len();
                    self.push(format!(
                        "provider · {} · {} · {} · models {}",
                        provider.display_name,
                        provider.id,
                        enum_label(provider.health.status),
                        model_count
                    ));
                }
                Ok(())
            }
            Some("show") => {
                let id = parse_provider_id(id.context("usage: /providers show <id>")?)?;
                let provider = self.application.store().get_provider_profile(id)?;
                self.push(format!(
                    "Provider {} · {} · {}",
                    provider.display_name,
                    provider.id,
                    enum_label(provider.health.status)
                ));
                self.push(format!(
                    "adapter {} · endpoint {} · enabled {} · credential {}",
                    enum_label(provider.adapter),
                    provider.endpoint_summary(),
                    provider.enabled,
                    if provider.credential_ref.is_some() {
                        "configured"
                    } else {
                        "not configured"
                    }
                ));
                if let Some(message) = provider.health.safe_message {
                    self.push(format!("health · {message}"));
                }
                Ok(())
            }
            Some("check") => {
                let id = parse_provider_id(id.context("usage: /providers check <id>")?)?;
                let provider = self.application.store().get_provider_profile(id)?;
                provider.validate()?;
                self.push(format!(
                    "passive Provider check · {} · configuration valid · cached status {}",
                    provider.display_name,
                    enum_label(provider.health.status)
                ));
                self.push(
                    "No billable request was sent. Use GUI Provider settings for a credential-aware network check; the TUI never accepts or prints API keys.",
                );
                Ok(())
            }
            Some(_) => {
                anyhow::bail!("usage: /providers | /providers show <id> | /providers check <id>")
            }
        }
    }

    async fn models(&mut self, action: Option<&str>, id: Option<&str>) -> Result<()> {
        match action {
            None => {
                let lines = registry_model_lines(self.application.as_ref())?;
                if lines.is_empty() {
                    self.push("No Model Profiles. Add and verify one in GUI Model settings.");
                }
                for line in lines {
                    self.push(format!("model · {line}"));
                }
                return Ok(());
            }
            Some("workers") => {
                let lines = model_lines(&workspace_settings(self.application.as_ref())?);
                for line in lines {
                    self.push(format!("Vision Worker · {line}"));
                }
                return Ok(());
            }
            Some("show") => {
                let model_id =
                    parse_model_profile_id(id.context("usage: /models show <model-profile-id>")?)?;
                let model = self.application.store().get_model_profile(model_id, None)?;
                let provider = self
                    .application
                    .store()
                    .get_provider_profile(model.provider_id)?;
                self.push(format!(
                    "Model {} · {} · revision {} · {}",
                    model.display_name,
                    model.id,
                    model.revision,
                    enum_label(model.status)
                ));
                self.push(format!(
                    "Provider {} · inputs {} · capabilities {}",
                    provider.display_name,
                    model
                        .input_modalities
                        .iter()
                        .map(|value| enum_label(*value))
                        .collect::<Vec<_>>()
                        .join(", "),
                    model
                        .task_capabilities
                        .iter()
                        .map(|value| enum_label(*value))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                self.push(format!(
                    "protocol · Tool Calls {} · Structured Output {} · JSON Schema {}",
                    model.protocol_features.tool_calls,
                    model.protocol_features.structured_output,
                    model.protocol_features.json_schema
                ));
                return Ok(());
            }
            Some("compatible") => {
                let capability =
                    parse_model_capability(id.context("usage: /models compatible <capability>")?)?;
                let models = self.application.store().list_model_profiles(None, false)?;
                let mut count = 0_usize;
                for model in models {
                    let provider = self
                        .application
                        .store()
                        .get_provider_profile(model.provider_id)?;
                    if model.enabled
                        && model.status == ModelProfileStatus::Available
                        && model.task_capabilities.contains(&capability)
                        && provider.enabled
                        && matches!(
                            provider.health.status,
                            ProviderHealthStatus::Available | ProviderHealthStatus::Configured
                        )
                        && provider.credential_ref.is_some()
                    {
                        count += 1;
                        self.push(format!(
                            "compatible · {} via {} · {}",
                            model.display_name, provider.display_name, model.id
                        ));
                    }
                }
                if count == 0 {
                    self.push(format!(
                        "No Available Model Profile declares {}. Verify Provider credentials and model capabilities in the GUI.",
                        enum_label(capability)
                    ));
                }
                return Ok(());
            }
            Some("test") => {}
            Some(_) => anyhow::bail!(
                "usage: /models | /models show <id> | /models compatible <capability> | /models workers | /models test <worker-id>"
            ),
        }
        let settings = workspace_settings(self.application.as_ref())?;
        let id = id.context("usage: /models test <id>")?;
        let worker = settings
            .detection_workers
            .iter()
            .find(|worker| worker.id == id || worker.model_id == id)
            .context("unknown Detection Worker id")?;
        if !worker.enabled {
            self.push(format!(
                "{} is disabled; enable it in Settings before testing.",
                worker.display_name
            ));
            return Ok(());
        }
        let started = std::time::Instant::now();
        let client = HttpVisionWorkerClient::new(worker.http_config()?)?;
        match client.health().await {
            Ok(health) => {
                let latency = started.elapsed().as_millis();
                let capability = worker_capability(worker);
                let no_score =
                    if worker.score_semantics == annotagent_core::ScoreSemantics::NotProvided {
                        " · No confidence score"
                    } else {
                        ""
                    };
                let line = format!(
                    "{} · {} · {:?} · {latency} ms{no_score}",
                    worker.display_name, capability, health.status
                );
                self.model_lines
                    .retain(|item| !item.starts_with(&worker.display_name));
                self.model_lines.push(line.clone());
                self.push(format!("model test · {line}"));
            }
            Err(error) => self.push(format!(
                "model test · {} unavailable · {error}",
                worker.display_name
            )),
        }
        Ok(())
    }

    fn bindings(&mut self) -> Result<()> {
        let Some(project) = self.project_context.as_ref() else {
            self.push("Open a Project before viewing model bindings.");
            return Ok(());
        };
        let project_name = project.name.clone();
        let stable_id = project.stable_id;
        let bindings = self
            .application
            .store()
            .list_project_model_bindings(stable_id)?;
        self.push(format!("Project model bindings · {project_name}"));
        if bindings.is_empty() {
            self.push("No Project defaults; Workflow nodes and global defaults apply.");
        }
        for binding in bindings {
            let model = self
                .application
                .store()
                .get_model_profile(binding.model_profile_id, None)?;
            self.push(format!(
                "{} · {} · {} · locked {}",
                enum_label(binding.role),
                model.display_name,
                binding.model_profile_id,
                binding.locked
            ));
        }
        let defaults = self.application.store().get_global_model_defaults()?;
        self.push(format!(
            "global · Pipeline Builder {} · vision {} · text {}",
            defaults
                .pipeline_builder
                .map_or_else(|| "none".to_owned(), |id| id.to_string()),
            defaults
                .vision_language
                .map_or_else(|| "none".to_owned(), |id| id.to_string()),
            defaults
                .text_generation
                .map_or_else(|| "none".to_owned(), |id| id.to_string())
        ));
        Ok(())
    }

    fn bind(&mut self, role: Option<&str>, model_id: Option<&str>) -> Result<()> {
        let role =
            parse_model_binding_role(role.context("usage: /bind <role> <model-profile-id>")?)?;
        let model_id =
            parse_model_profile_id(model_id.context("usage: /bind <role> <model-profile-id>")?)?;
        let project_id = self
            .project_context
            .as_ref()
            .map(|project| project.stable_id)
            .context("open a Project before choosing a model")?;
        let model = self.application.store().get_model_profile(model_id, None)?;
        let capability = binding_capability(role, &model)?;
        if role == ModelBindingRole::PipelineBuilder
            && (!model.input_modalities.contains(&InputModality::Text)
                || !model.protocol_features.tool_calls
                || !model.protocol_features.structured_output)
        {
            anyhow::bail!(
                "Pipeline Builder requires text input, Tool Calls, and Structured Output"
            );
        }
        let binding = ProjectModelBinding {
            id: ModelBindingId::new(),
            project_id,
            capability,
            role,
            match_kind: ModelBindingMatch::Role,
            model_profile_id: model_id,
            locked: true,
            created_at: chrono::Utc::now(),
        };
        self.application
            .store()
            .save_project_model_binding(&binding, BindingMutationActor::User)?;
        self.push(format!(
            "bound {} to {} · locked for Agent replacement",
            enum_label(role),
            model.display_name
        ));
        Ok(())
    }

    async fn command(&mut self, command: &str) -> Result<()> {
        let mut parts = command.split_whitespace();
        match parts.next().unwrap_or_default() {
            "/run" => self.start(),
            "/pause" => {
                if let Some(active) = &self.active {
                    self.application.pause_run(active.run_id).await?;
                }
                Ok(())
            }
            "/resume" | "/retry" => {
                if let Some(active) = &self.active {
                    self.application.resume_run(active.run_id).await?;
                }
                Ok(())
            }
            "/cancel" => {
                self.cancel().await;
                Ok(())
            }
            "/open" => {
                if self.active.is_some() {
                    self.push("Finish or cancel the active Run before opening another Project.");
                    return Ok(());
                }
                let path = parts.next().context("usage: /open <project.yaml>")?;
                let path = PathBuf::from(path);
                let context = ProjectContext::load(&path)?;
                let workspace = project_workspace(&path)?;
                self.application = Arc::new(LocalApplication::with_database(
                    workspace,
                    runner::database_path()?,
                )?);
                self.model_lines = registry_model_lines(self.application.as_ref())?
                    .into_iter()
                    .chain(model_lines(&workspace_settings(self.application.as_ref())?))
                    .collect();
                self.push(format!("opened Project {}", context.name));
                self.project = Some(path);
                self.project_context = Some(context);
                Ok(())
            }
            "/init" => {
                self.push("Create a Project with: annotagent init <directory> --skill <skill-id>");
                Ok(())
            }
            "/history" | "/trace" => {
                let store = annotagent_storage::SqliteStore::open(runner::database_path()?)?;
                for run in store.list_runs()?.into_iter().take(8) {
                    self.push(format!("{} {:?} {}", run.id, run.status, run.project_name));
                }
                if let Some(project) = &self.project_context {
                    for session in self
                        .application
                        .list_agent_sessions(&project.id)?
                        .into_iter()
                        .take(8)
                    {
                        self.push(format!(
                            "agent {} {:?} · tools {} · cost {} · stop {}",
                            session.id,
                            session.status,
                            session.usage.tool_calls,
                            session.usage.cost,
                            session.stop_reason.as_deref().unwrap_or("running")
                        ));
                        if let Some(selection) = &session.model_selection {
                            self.push(format!(
                                "Agent model · {} via {} · revision {} · {}",
                                selection.model_display_name,
                                selection.provider_display_name,
                                selection.model_profile_revision,
                                enum_label(selection.binding_source)
                            ));
                        }
                    }
                }
                Ok(())
            }
            "/inspect" => self.inspect_latest(),
            "/artifacts" => self.list_artifacts(),
            "/replay" => self.replay(parts.next()).await,
            "/providers" => self.providers(parts.next(), parts.next()),
            "/models" => self.models(parts.next(), parts.next()).await,
            "/bindings" => self.bindings(),
            "/bind" => self.bind(parts.next(), parts.next()),
            "/skills" => {
                if parts.next() == Some("show") {
                    let id = parts.next().context("usage: /skills show <skill-id>")?;
                    let skill = self.application.layered_skills().get(id)?;
                    let manifest = skill.manifest();
                    self.push(format!(
                        "Skill {}@{} · {:?} · {}",
                        manifest.id, manifest.skill_version, manifest.kind, manifest.display_name
                    ));
                    self.push(format!(
                        "nodes {} · validators {} · policies {} · resources {}",
                        manifest.nodes.join(", "),
                        manifest.validators.join(", "),
                        manifest.policies.join(", "),
                        manifest.summary_resources.join(", ")
                    ));
                } else {
                    for skill in self.application.layered_skills().catalog() {
                        self.push(format!(
                            "{:?} · {}@{} · {}",
                            skill.kind, skill.id, skill.version, skill.display_name
                        ));
                    }
                }
                Ok(())
            }
            "/advisor" => {
                let project_id = self
                    .project_context
                    .as_ref()
                    .map(|project| project.id.clone())
                    .context("open a Project before running Advisor")?;
                let action = parts.next();
                if action == Some("status") {
                    let session = self
                        .application
                        .list_agent_sessions(&project_id)?
                        .into_iter()
                        .find(|session| {
                            matches!(
                                session.kind,
                                annotagent_core::AgentKind::PipelineBuilder
                                    | annotagent_core::AgentKind::WorkflowAdvisor
                            )
                        })
                        .context("no Pipeline Builder Session")?;
                    self.push(format!(
                        "Pipeline Builder {} · {:?} · tools {}/{} · tokens {} · cost {}",
                        session.id,
                        session.status,
                        session.usage.tool_calls,
                        session.budget.max_tool_calls,
                        session.usage.input_tokens + session.usage.output_tokens,
                        session.usage.cost
                    ));
                    if let Some(selection) = &session.model_selection {
                        self.push(format!(
                            "Agent model · {} via {} · revision {} · {}{}",
                            selection.model_display_name,
                            selection.provider_display_name,
                            selection.model_profile_revision,
                            enum_label(selection.binding_source),
                            if selection.locked { " · locked" } else { "" }
                        ));
                    }
                    if let Some(constraints) = &session.builder_constraints {
                        self.push(format!(
                            "objective {:?} · review target {} · external APIs {} · human review {}",
                            constraints.priority,
                            constraints
                                .target_review_rate
                                .map_or_else(|| "any".to_owned(), |value| format!("{:.0}%", value * 100.0)),
                            constraints.allow_external_models,
                            constraints.allow_human_review,
                        ));
                    }
                    for step in &session.steps {
                        self.push(format!(
                            "{}. {} · {}",
                            step.sequence,
                            step.tool_name,
                            if step.success { "completed" } else { "failed" }
                        ));
                    }
                    return Ok(());
                }
                if action == Some("cancel") {
                    let session = self
                        .application
                        .list_agent_sessions(&project_id)?
                        .into_iter()
                        .find(|session| {
                            matches!(
                                session.kind,
                                annotagent_core::AgentKind::PipelineBuilder
                                    | annotagent_core::AgentKind::WorkflowAdvisor
                            ) && matches!(
                                session.status,
                                AgentSessionStatus::Running | AgentSessionStatus::WaitingForHuman
                            )
                        })
                        .context("no cancellable Advisor Session")?;
                    let session = self.application.cancel_agent_session(session.id)?;
                    self.push(format!(
                        "Advisor {} cancelled · {}",
                        session.id,
                        session.stop_reason.as_deref().unwrap_or("cancelled")
                    ));
                    return Ok(());
                }
                self.push(
                    "Advisor started · scripted offline mode · registry-bounded Draft only. Use /bind pipeline_builder <model-profile-id> to select the live GUI Agent model.",
                );
                let report = self
                    .application
                    .run_workflow_advisor_agent(
                        &project_id,
                        &load_settings(None)?,
                        &annotagent_core::WorkflowConstraints::default(),
                        None,
                        PipelineBuilderConstraints::default(),
                        CancellationToken::new(),
                    )
                    .await?;
                for step in &report.session.steps {
                    self.push(format!(
                        "agent tool {} · {}",
                        step.tool_name,
                        if step.success { "completed" } else { "failed" }
                    ));
                }
                self.push(format!(
                    "Advisor {:?} · tokens {} · cost {} · stop {}",
                    report.session.status,
                    report.session.usage.input_tokens + report.session.usage.output_tokens,
                    report.session.usage.cost,
                    report.session.stop_reason.as_deref().unwrap_or("running")
                ));
                Ok(())
            }
            "/memory" => {
                let project_id = self
                    .project_context
                    .as_ref()
                    .map(|project| project.id.clone())
                    .context("open a Project before viewing Memory")?;
                let records = self
                    .application
                    .list_project_correction_memory(&project_id)?;
                if records.is_empty() {
                    self.push("No Project-scoped correction evidence.");
                }
                for record in records.into_iter().take(20) {
                    self.push(format!(
                        "memory {} · {} · task {} · Label {}",
                        record.skill_id,
                        record.reason_code,
                        record.task_id,
                        record
                            .predicted_label
                            .as_ref()
                            .map_or("any", annotagent_core::LabelId::as_str)
                    ));
                }
                Ok(())
            }
            "/gui" => {
                webbrowser::open("http://127.0.0.1:8787")?;
                self.push("opened GUI at http://127.0.0.1:8787");
                Ok(())
            }
            "/config" => {
                self.push("config/default.toml · secrets are read from the configured env var");
                Ok(())
            }
            "/help" | "?" => {
                self.push("/open /init /skills /providers /providers show <id> /providers check <id> /models /models show <id> /models compatible <capability> /models workers /models test <worker-id> /bindings /bind <role> <model-profile-id> /advisor /advisor status /advisor cancel /run /pause /resume /cancel /replay [node] /artifacts /memory /history /trace /inspect /config /gui /help /quit");
                Ok(())
            }
            "/quit" | "/q" => {
                self.quit = true;
                Ok(())
            }
            unknown => {
                self.push(format!("unknown command {unknown:?}; enter /help"));
                Ok(())
            }
        }
    }
}

fn workspace_settings(application: &LocalApplication) -> Result<Settings> {
    let path = application.workspace().join(".annotagent/settings.toml");
    load_settings(path.is_file().then_some(path.as_path()))
}

fn enum_label<T: serde::Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned())
}

fn parse_provider_id(value: &str) -> Result<ProviderId> {
    value
        .parse()
        .with_context(|| format!("invalid Provider Profile id {value:?}"))
}

fn parse_model_profile_id(value: &str) -> Result<ModelProfileId> {
    value
        .parse()
        .with_context(|| format!("invalid Model Profile id {value:?}"))
}

fn parse_model_capability(value: &str) -> Result<ModelCapability> {
    serde_json::from_value(serde_json::Value::String(value.to_owned()))
        .with_context(|| format!("unknown model capability {value:?}"))
}

fn parse_model_binding_role(value: &str) -> Result<ModelBindingRole> {
    serde_json::from_value(serde_json::Value::String(value.to_owned()))
        .with_context(|| format!("unknown model binding role {value:?}"))
}

fn binding_capability(
    role: ModelBindingRole,
    model: &annotagent_core::ModelProfile,
) -> Result<ModelCapability> {
    let required = match role {
        ModelBindingRole::PipelineBuilder => Some(ModelCapability::TextGeneration),
        ModelBindingRole::Detection => Some(ModelCapability::ObjectDetection),
        ModelBindingRole::Classification => Some(ModelCapability::ImageClassification),
        ModelBindingRole::Segmentation => Some(ModelCapability::InstanceSegmentation),
        ModelBindingRole::Verification => Some(ModelCapability::VisionLanguage),
        ModelBindingRole::PrimaryInference | ModelBindingRole::Fallback => None,
    };
    let capability = required
        .or_else(|| model.task_capabilities.iter().next().copied())
        .context("Model Profile declares no task capability")?;
    if !model.task_capabilities.contains(&capability) {
        anyhow::bail!(
            "{} requires the {} capability, which {} does not declare",
            enum_label(role),
            enum_label(capability),
            model.display_name
        );
    }
    Ok(capability)
}

fn registry_model_lines(application: &LocalApplication) -> Result<Vec<String>> {
    application
        .store()
        .list_model_profiles(None, false)?
        .into_iter()
        .map(|model| {
            let provider = application
                .store()
                .get_provider_profile(model.provider_id)?;
            Ok(format!(
                "{} · {} · {} · {}",
                model.display_name,
                provider.display_name,
                enum_label(model.status),
                model.id
            ))
        })
        .collect()
}

fn worker_capability(worker: &DetectionWorkerSettings) -> String {
    worker.expected_capabilities.first().map_or_else(
        || "unknown".to_owned(),
        |capability| {
            serde_json::to_value(capability)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "unknown".to_owned())
        },
    )
}

fn model_lines(settings: &Settings) -> Vec<String> {
    settings
        .detection_workers
        .iter()
        .map(|worker| {
            let status = if worker.enabled {
                "Configured"
            } else {
                "Disabled"
            };
            let no_score = if worker.score_semantics == annotagent_core::ScoreSemantics::NotProvided
            {
                " · No confidence score"
            } else {
                ""
            };
            format!(
                "{} · {} · {status}{no_score}",
                worker.display_name,
                worker_capability(worker)
            )
        })
        .collect()
}

pub async fn run(project: Option<PathBuf>) -> Result<()> {
    let workspace = match project.as_deref() {
        Some(path) => project_workspace(path)?,
        None => std::env::current_dir()?.canonicalize()?,
    };
    let application = Arc::new(LocalApplication::with_database(
        workspace,
        runner::database_path()?,
    )?);
    enable_raw_mode().context("cannot enable terminal raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("cannot enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("cannot create terminal")?;
    let result = event_loop(&mut terminal, project, application).await;
    disable_raw_mode().context("cannot disable terminal raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("cannot leave alternate screen")?;
    terminal.show_cursor().context("cannot restore cursor")?;
    result
}

fn project_workspace(path: &Path) -> Result<PathBuf> {
    ProjectContext::load(path)?;
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .with_context(|| format!("cannot access Project workspace for {}", path.display()))
}

async fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    project: Option<PathBuf>,
    application: Arc<LocalApplication>,
) -> Result<()> {
    let mut state = TuiState::new(project, application)?;
    loop {
        state.drain_events();
        state.collect_finished().await;
        terminal.draw(|frame| draw(frame, &state))?;
        if state.quit {
            if let Some(active) = &state.active {
                let _ignored = state.application.cancel_run(active.run_id).await;
            }
            return Ok(());
        }
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('q') if state.input.is_empty() => state.quit = true,
                KeyCode::Char('r') if state.input.is_empty() => {
                    if let Err(error) = state.start() {
                        state.push(format!("cannot start: {error:#}"));
                    }
                }
                KeyCode::Char('c') if state.input.is_empty() => state.cancel().await,
                KeyCode::Char('g') if state.input.is_empty() => {
                    if let Err(error) = webbrowser::open("http://127.0.0.1:8787") {
                        state.push(format!("cannot open GUI: {error}"));
                    }
                }
                KeyCode::Char(' ') if state.input.is_empty() => state.pause_or_resume().await,
                KeyCode::Enter => {
                    let command = std::mem::take(&mut state.input);
                    if !command.is_empty()
                        && let Err(error) = state.command(&command).await
                    {
                        state.push(format!("command failed: {error:#}"));
                    }
                }
                KeyCode::Backspace => {
                    state.input.pop();
                }
                KeyCode::Char(character) => state.input.push(character),
                KeyCode::Esc => state.input.clear(),
                _ => {}
            }
        }
    }
}

fn draw(frame: &mut ratatui::Frame<'_>, state: &TuiState) {
    let theme = AnnotAgentTheme::detect();
    let area = frame.area();
    frame.render_widget(Block::default().style(theme.base()), area);
    if area.width < 48 || area.height < 12 {
        draw_tiny(frame, state, theme);
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(4),
            Constraint::Min(8),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(" AnnotAgent", theme.title())),
            Line::from(Span::styled(
                " Composable Annotation Agent Runtime",
                theme.muted(),
            )),
        ])
        .style(theme.base()),
        rows[0],
    );

    if area.width < 90 {
        frame.render_widget(
            Paragraph::new(format!(
                "{} · task {} · status {} · {}",
                project_summary(state),
                state.current_task,
                status_label(state.status),
                state.usage
            ))
            .style(theme.panel())
            .block(themed_block("Workspace", theme))
            .wrap(Wrap { trim: true }),
            rows[1],
        );
    } else {
        let header = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(35),
                Constraint::Percentage(40),
                Constraint::Percentage(25),
            ])
            .split(rows[1]);
        frame.render_widget(
            Paragraph::new(project_summary(state))
                .style(theme.panel())
                .block(themed_block("Project", theme))
                .wrap(Wrap { trim: true }),
            header[0],
        );
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(format!("task {} · ", state.current_task)),
                Span::styled(
                    status_label(state.status),
                    theme.status(status_tone(state.status)),
                ),
            ]))
            .style(theme.panel())
            .block(themed_block("Current", theme)),
            header[1],
        );
        frame.render_widget(
            Paragraph::new(state.usage.as_str())
                .style(theme.panel())
                .block(themed_block("Usage / Budget", theme)),
            header[2],
        );
    }

    let body = if area.width >= 110 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(rows[2])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(rows[2])
    };
    let visible = usize::from(body[0].height.saturating_sub(2));
    let trace = state
        .trace
        .iter()
        .rev()
        .take(visible)
        .rev()
        .map(|entry| trace_line(entry, theme))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(trace)
            .style(theme.panel())
            .block(themed_block(
                "Agent Trace · visible execution events",
                theme,
            ))
            .wrap(Wrap { trim: false }),
        body[0],
    );
    if area.width >= 110 {
        let models = state
            .model_lines
            .iter()
            .flat_map(|entry| [Line::from(entry.as_str()), Line::default()])
            .collect::<Vec<_>>();
        frame.render_widget(
            Paragraph::new(models)
                .style(theme.panel())
                .block(themed_block("Models · /models · /providers", theme))
                .wrap(Wrap { trim: true }),
            body[1],
        );
    }
    frame.render_widget(
        Paragraph::new(format!("> {}", state.input))
            .style(theme.command())
            .block(themed_block("Command", theme)),
        rows[3],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" r ", theme.selected()),
            Span::raw(" run · Space pause/resume · c cancel · g GUI · q quit · /help"),
        ]))
        .style(theme.base()),
        rows[4],
    );
}

fn draw_tiny(frame: &mut ratatui::Frame<'_>, state: &TuiState, theme: AnnotAgentTheme) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled("AnnotAgent", theme.title())),
            Line::from(Span::styled(
                "Composable Annotation Agent Runtime",
                theme.muted(),
            )),
            Line::from(project_summary(state)),
            Line::from(vec![
                Span::raw(format!("{} · ", state.current_task)),
                Span::styled(
                    status_label(state.status),
                    theme.status(status_tone(state.status)),
                ),
            ]),
            Line::styled(
                "r run · Space pause/resume · c cancel · q quit",
                theme.muted(),
            ),
        ])
        .style(theme.panel_muted())
        .block(themed_block("AnnotAgent", theme))
        .wrap(Wrap { trim: true }),
        frame.area(),
    );
}

fn project_summary(state: &TuiState) -> String {
    match (&state.project, &state.project_context) {
        (Some(path), Some(context)) => format!(
            "Project: {} · Workflow: {} · Skills: {} · {}",
            context.name,
            context.workflow,
            context.skills,
            path.display()
        ),
        _ => "No project opened · Use /open or /init".to_owned(),
    }
}

fn themed_block(title: &str, theme: AnnotAgentTheme) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border())
        .title(Span::styled(title, theme.title()))
}

fn status_label(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Pending => "Draft",
        RunStatus::Running => "Running",
        RunStatus::Paused => "Paused",
        RunStatus::AwaitingReview => "Awaiting review",
        RunStatus::CompletedWithReview => "Completed with review",
        RunStatus::Partial => "Partial",
        RunStatus::Completed => "Completed",
        RunStatus::Cancelled => "Rejected",
        RunStatus::BudgetExceeded | RunStatus::Failed => "Failed",
        RunStatus::Interrupted => "Interrupted",
    }
}

fn status_tone(status: RunStatus) -> StatusTone {
    match status {
        RunStatus::Pending => StatusTone::Neutral,
        RunStatus::Running | RunStatus::Paused => StatusTone::Running,
        RunStatus::Completed => StatusTone::Success,
        RunStatus::AwaitingReview | RunStatus::CompletedWithReview | RunStatus::Partial => {
            StatusTone::Warning
        }
        RunStatus::Cancelled
        | RunStatus::BudgetExceeded
        | RunStatus::Failed
        | RunStatus::Interrupted => StatusTone::Danger,
    }
}

fn trace_line(entry: &str, theme: AnnotAgentTheme) -> Line<'_> {
    let tone = if entry.starts_with("tool ") || entry.starts_with("validation:") {
        StatusTone::Info
    } else if entry.contains("failed") || entry.contains("error") {
        StatusTone::Danger
    } else {
        StatusTone::Neutral
    };
    Line::styled(entry, theme.status(tone))
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    #[test]
    fn normal_and_small_terminal_layouts_render_without_panicking() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = Arc::new(LocalApplication::new(temporary.path()).expect("application"));
        let state = TuiState::new(None, application).expect("state");
        for (width, height) in [(120, 32), (80, 20), (40, 8)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("test terminal");
            terminal
                .draw(|frame| draw(frame, &state))
                .expect("draw succeeds");
        }
    }

    #[test]
    fn no_project_state_is_generic_and_actionable() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = Arc::new(LocalApplication::new(temporary.path()).expect("application"));
        let state = TuiState::new(None, application).expect("state");
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| draw(frame, &state))
            .expect("draw succeeds");
        let contents = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(contents.contains("AnnotAgent"));
        assert!(contents.contains("No project opened"));
        assert!(contents.contains("Composable Annotation Agent Runtime"));
        assert!(!contents.contains("RoboCup"));
    }

    #[test]
    fn project_context_names_the_active_skill() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = Arc::new(LocalApplication::new(temporary.path()).expect("application"));
        let project =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/robocup/project.yaml");
        let state = TuiState::new(Some(project), application).expect("state");
        let summary = project_summary(&state);
        assert!(summary.contains("Project:"));
        assert!(summary.contains("Workflow: Configured task graph@v1"));
        assert!(summary.contains("Skills: robocup"));
    }

    #[test]
    fn status_labels_are_not_color_only() {
        assert_eq!(status_label(RunStatus::Pending), "Draft");
        assert_eq!(
            status_label(RunStatus::CompletedWithReview),
            "Completed with review"
        );
        assert_eq!(status_label(RunStatus::Completed), "Completed");
        assert_eq!(status_label(RunStatus::Failed), "Failed");
    }

    #[test]
    fn models_panel_explains_capability_availability_and_missing_scores() {
        let settings = load_settings(None).expect("default Settings");
        let lines = model_lines(&settings);
        assert!(lines.iter().any(|line| {
            line.contains("LocateAnything")
                && line.contains("open_vocabulary_detection")
                && line.contains("No confidence score")
        }));
        assert!(
            lines
                .iter()
                .any(|line| { line.contains("RF-DETR") && line.contains("object_detection") })
        );
    }

    #[tokio::test]
    async fn registry_commands_are_safe_and_actionable_without_profiles() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = Arc::new(LocalApplication::new(temporary.path()).expect("application"));
        let mut state = TuiState::new(None, application).expect("state");
        state.command("/providers").await.expect("Provider list");
        state.command("/models").await.expect("Model list");
        state.command("/bindings").await.expect("binding list");
        assert!(
            state
                .trace
                .iter()
                .any(|line| line.contains("GUI") && line.contains("credentials"))
        );
        assert!(
            state
                .trace
                .iter()
                .any(|line| line.contains("No Model Profiles"))
        );
        assert!(
            state
                .trace
                .iter()
                .any(|line| line.contains("Open a Project"))
        );
        assert!(
            state
                .trace
                .iter()
                .all(|line| !line.to_ascii_lowercase().contains("authorization"))
        );
    }

    #[tokio::test]
    async fn skill_memory_and_advisor_cancel_commands_use_persisted_application_state() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let project_root = temporary.path().join("demo");
        std::fs::create_dir_all(project_root.join("images")).expect("Project directory");
        std::fs::write(
            project_root.join("project.yaml"),
            r"
version: 1
project:
  name: Agent command demo
  language: en
dataset:
  root: images
runtime: {}
tasks: []
review:
  auto_accept_confidence: 0.9
  force_review_below: 0.5
export:
  formats: [native]
",
        )
        .expect("Project schema");
        let application = Arc::new(LocalApplication::new(temporary.path()).expect("application"));
        let mut session = annotagent_core::AgentSession::start(
            annotagent_core::AgentKind::WorkflowAdvisor,
            annotagent_core::AgentBudget::default(),
        )
        .with_project("demo");
        session.wait_for_human("publish_workflow");
        application
            .store()
            .save_agent_session(&session)
            .expect("Advisor Session");
        let mut state = TuiState::new(Some(project_root.join("project.yaml")), application.clone())
            .expect("TUI state");
        state
            .command("/skills show classification")
            .await
            .expect("Skill detail");
        state.command("/memory").await.expect("Memory list");
        state
            .command("/advisor cancel")
            .await
            .expect("Advisor cancel");
        assert!(
            state
                .trace
                .iter()
                .any(|line| line.contains("Classification"))
        );
        assert!(
            state
                .trace
                .iter()
                .any(|line| line.contains("No Project-scoped correction"))
        );
        assert_eq!(
            application
                .store()
                .get_agent_session(session.id)
                .expect("cancelled Session")
                .status,
            AgentSessionStatus::Cancelled
        );
    }
}
