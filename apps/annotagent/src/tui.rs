use std::{io, path::PathBuf, time::Duration};

use annotagent_core::{RunEvent, RunEventPayload, RunStatus};
use annotagent_runtime::RunControl;
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
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio::{sync::broadcast, task::JoinHandle};

use crate::runner;

struct ActiveRun {
    control: RunControl,
    events: broadcast::Receiver<RunEvent>,
    handle:
        JoinHandle<Result<annotagent_runtime::ImageRunResult, annotagent_runtime::RuntimeError>>,
}

struct TuiState {
    project: PathBuf,
    input: String,
    trace: Vec<String>,
    status: RunStatus,
    current_task: String,
    usage: String,
    active: Option<ActiveRun>,
    quit: bool,
}

impl TuiState {
    fn new(project: PathBuf) -> Self {
        Self {
            project,
            input: String::new(),
            trace: vec!["Ready. Press r or enter /run to start the deterministic demo.".to_owned()],
            status: RunStatus::Pending,
            current_task: "-".to_owned(),
            usage: "input 0 · output 0 · cost 0".to_owned(),
            active: None,
            quit: false,
        }
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
        let prepared = runner::prepare_run(&self.project, "mock", None)?;
        let mut events = prepared.runtime.event_bus().subscribe();
        // Ensure the subscription is live before the task emits RunCreated.
        while events.try_recv().is_ok() {}
        let control = prepared.runtime.control();
        let runtime = prepared.runtime;
        let request = prepared.request;
        let handle = tokio::spawn(async move { runtime.run_image(request).await });
        self.active = Some(ActiveRun {
            control,
            events,
            handle,
        });
        self.push(format!("Started image {}", prepared.image_path.display()));
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
            RunEventPayload::Annotation { summary, .. } | RunEventPayload::Message { summary } => {
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
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.handle.is_finished())
            && let Some(active) = self.active.take()
        {
            match active.handle.await {
                Ok(Ok(result)) => {
                    self.status = result.status;
                    self.push(format!(
                        "finished: committed {}, review {}, issues {}",
                        result.committed.len(),
                        result.review_queue.len(),
                        result.issues.len()
                    ));
                }
                Ok(Err(error)) => self.push(format!("run failed: {error}")),
                Err(error) => self.push(format!("run task failed: {error}")),
            }
        }
    }

    fn pause_or_resume(&mut self) {
        let Some(active) = &self.active else {
            self.push("No active run.");
            return;
        };
        let result = if active.control.status().ok() == Some(RunStatus::Paused) {
            active.control.resume().map(|_| "resumed")
        } else {
            active.control.pause().map(|_| "paused")
        };
        match result {
            Ok(message) => self.push(message),
            Err(error) => self.push(format!("control error: {error}")),
        }
    }

    fn cancel(&mut self) {
        let Some(active) = &self.active else {
            self.push("No active run.");
            return;
        };
        match active.control.cancel() {
            Ok(_) => self.push("cancellation requested"),
            Err(error) => self.push(format!("control error: {error}")),
        }
    }

    fn command(&mut self, command: &str) -> Result<()> {
        let mut parts = command.split_whitespace();
        match parts.next().unwrap_or_default() {
            "/run" => self.start(),
            "/pause" => {
                if let Some(active) = &self.active {
                    active.control.pause()?;
                }
                Ok(())
            }
            "/resume" | "/retry" => {
                if let Some(active) = &self.active {
                    active.control.resume()?;
                }
                Ok(())
            }
            "/cancel" => {
                self.cancel();
                Ok(())
            }
            "/open" => {
                let path = parts.next().context("usage: /open <project.yaml>")?;
                let path = PathBuf::from(path);
                runner::load_project(&path)?;
                self.project = path;
                self.push("project opened");
                Ok(())
            }
            "/history" | "/trace" => {
                let store = annotagent_storage::SqliteStore::open(runner::database_path()?)?;
                for run in store.list_runs()?.into_iter().take(8) {
                    self.push(format!("{} {:?} {}", run.id, run.status, run.project_name));
                }
                Ok(())
            }
            "/skills" => {
                self.push("robocup · RoboCup Perception Annotation");
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
                self.push("/open /run /pause /resume /cancel /retry /history /trace /config /skills /gui /help /quit");
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

pub async fn run(project: PathBuf) -> Result<()> {
    runner::load_project(&project)?;
    enable_raw_mode().context("cannot enable terminal raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("cannot enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("cannot create terminal")?;
    let result = event_loop(&mut terminal, project).await;
    disable_raw_mode().context("cannot disable terminal raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("cannot leave alternate screen")?;
    terminal.show_cursor().context("cannot restore cursor")?;
    result
}

async fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    project: PathBuf,
) -> Result<()> {
    let mut state = TuiState::new(project);
    loop {
        state.drain_events();
        state.collect_finished().await;
        terminal.draw(|frame| draw(frame, &state))?;
        if state.quit {
            if let Some(active) = &state.active {
                let _ignored = active.control.cancel();
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
                KeyCode::Char('c') if state.input.is_empty() => state.cancel(),
                KeyCode::Char('g') if state.input.is_empty() => {
                    if let Err(error) = webbrowser::open("http://127.0.0.1:8787") {
                        state.push(format!("cannot open GUI: {error}"));
                    }
                }
                KeyCode::Char(' ') if state.input.is_empty() => state.pause_or_resume(),
                KeyCode::Enter => {
                    let command = std::mem::take(&mut state.input);
                    if !command.is_empty()
                        && let Err(error) = state.command(&command)
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
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(8),
            Constraint::Length(4),
            Constraint::Length(1),
        ])
        .split(frame.area());
    let header = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(40),
            Constraint::Percentage(25),
        ])
        .split(rows[0]);
    frame.render_widget(
        Paragraph::new(state.project.display().to_string())
            .block(Block::default().borders(Borders::ALL).title("Project"))
            .wrap(Wrap { trim: true }),
        header[0],
    );
    frame.render_widget(
        Paragraph::new(format!("task {} · {:?}", state.current_task, state.status))
            .block(Block::default().borders(Borders::ALL).title("Current")),
        header[1],
    );
    frame.render_widget(
        Paragraph::new(state.usage.as_str()).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Usage / Budget"),
        ),
        header[2],
    );
    let visible = usize::from(rows[1].height.saturating_sub(2));
    let trace = state
        .trace
        .iter()
        .rev()
        .take(visible)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    frame.render_widget(
        Paragraph::new(trace)
            .block(Block::default().borders(Borders::ALL).title("Agent Trace"))
            .wrap(Wrap { trim: false }),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(format!("> {}", state.input))
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::ALL).title("Command")),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("r", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" run · Space pause/resume · c cancel · g GUI · q quit · /help"),
        ])),
        rows[3],
    );
}
