use std::{io, path::PathBuf, sync::Arc, time::Duration};

use annotagent_application::{AnnotAgentApplication, LocalApplication};
use annotagent_core::{RunEvent, RunEventPayload, RunStatus};
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

use crate::runner;

mod theme;

use theme::{AnnotAgentTheme, StatusTone};

struct ActiveRun {
    run_id: annotagent_core::RunId,
    events: broadcast::Receiver<RunEvent>,
}

struct TuiState {
    project: PathBuf,
    application: Arc<LocalApplication>,
    input: String,
    trace: Vec<String>,
    status: RunStatus,
    current_task: String,
    usage: String,
    active: Option<ActiveRun>,
    quit: bool,
}

impl TuiState {
    fn new(project: PathBuf, application: Arc<LocalApplication>) -> Self {
        Self {
            project,
            application,
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
        let mut events = self.application.subscribe();
        while events.try_recv().is_ok() {}
        let started = self
            .application
            .start_run_path(&self.project, "mock", None)?;
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
        if matches!(
            self.status,
            RunStatus::AwaitingReview
                | RunStatus::Completed
                | RunStatus::Cancelled
                | RunStatus::BudgetExceeded
                | RunStatus::Failed
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
    let workspace = project
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .canonicalize()?;
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

async fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    project: PathBuf,
    application: Arc<LocalApplication>,
) -> Result<()> {
    let mut state = TuiState::new(project, application);
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
            Line::from(Span::styled(" RoboCup AnnotAgent", theme.title())),
            Line::from(vec![
                Span::styled(" AnnotAgent Core", theme.muted()),
                Span::styled(" · RoboCup Skill", theme.skill()),
            ]),
        ])
        .style(theme.base()),
        rows[0],
    );

    if area.width < 90 {
        frame.render_widget(
            Paragraph::new(format!(
                "{} · task {} · status {} · {}",
                state.project.display(),
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
            Paragraph::new(state.project.display().to_string())
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

    let visible = usize::from(rows[2].height.saturating_sub(2));
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
        rows[2],
    );
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
            Line::from(Span::styled("RoboCup AnnotAgent", theme.title())),
            Line::from(vec![
                Span::styled("AnnotAgent Core", theme.muted()),
                Span::styled(" · RoboCup Skill", theme.skill()),
            ]),
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
        .block(themed_block("AnnotAgent Core", theme))
        .wrap(Wrap { trim: true }),
        frame.area(),
    );
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
        RunStatus::AwaitingReview => "Needs review",
        RunStatus::Completed => "Auto accepted",
        RunStatus::Cancelled => "Rejected",
        RunStatus::BudgetExceeded | RunStatus::Failed => "Failed",
    }
}

fn status_tone(status: RunStatus) -> StatusTone {
    match status {
        RunStatus::Pending => StatusTone::Neutral,
        RunStatus::Running | RunStatus::Paused => StatusTone::Running,
        RunStatus::Completed => StatusTone::Success,
        RunStatus::AwaitingReview => StatusTone::Warning,
        RunStatus::Cancelled | RunStatus::BudgetExceeded | RunStatus::Failed => StatusTone::Danger,
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
        let state = TuiState::new(temporary.path().join("project.yaml"), application);
        for (width, height) in [(120, 32), (80, 20), (40, 8)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("test terminal");
            terminal
                .draw(|frame| draw(frame, &state))
                .expect("draw succeeds");
        }
    }

    #[test]
    fn status_labels_are_not_color_only() {
        assert_eq!(status_label(RunStatus::Pending), "Draft");
        assert_eq!(status_label(RunStatus::AwaitingReview), "Needs review");
        assert_eq!(status_label(RunStatus::Completed), "Auto accepted");
        assert_eq!(status_label(RunStatus::Failed), "Failed");
    }
}
