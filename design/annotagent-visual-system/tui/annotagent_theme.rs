//! Ready-to-adapt Ratatui theme for AnnotAgent.
//! Keep the runtime and domain types out of this module; map application states at the call site.

use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusTone {
    Neutral,
    Running,
    Success,
    Warning,
    Danger,
    Info,
}

#[derive(Debug, Clone, Copy)]
pub struct AnnotAgentTheme {
    pub bg: Color,
    pub surface: Color,
    pub surface_muted: Color,
    pub text: Color,
    pub text_muted: Color,
    pub border: Color,
    pub primary: Color,
    pub teal: Color,
    pub violet: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
}

impl Default for AnnotAgentTheme {
    fn default() -> Self {
        Self {
            bg: Color::Rgb(7, 17, 31),
            surface: Color::Rgb(11, 23, 42),
            surface_muted: Color::Rgb(18, 36, 58),
            text: Color::Rgb(248, 250, 252),
            text_muted: Color::Rgb(148, 163, 184),
            border: Color::Rgb(38, 58, 82),
            primary: Color::Rgb(96, 165, 250),
            teal: Color::Rgb(45, 212, 191),
            violet: Color::Rgb(167, 139, 250),
            success: Color::Rgb(74, 222, 128),
            warning: Color::Rgb(251, 191, 36),
            danger: Color::Rgb(248, 113, 113),
        }
    }
}

impl AnnotAgentTheme {
    pub fn base(self) -> Style {
        Style::default().fg(self.text).bg(self.bg)
    }

    pub fn panel(self) -> Style {
        Style::default().fg(self.text).bg(self.surface)
    }

    pub fn selected(self) -> Style {
        Style::default()
            .fg(Color::White)
            .bg(Color::Rgb(37, 99, 235))
            .add_modifier(Modifier::BOLD)
    }

    pub fn title(self) -> Style {
        Style::default().fg(self.teal).add_modifier(Modifier::BOLD)
    }

    pub fn muted(self) -> Style {
        Style::default().fg(self.text_muted)
    }

    pub fn status(self, tone: StatusTone) -> Style {
        let color = match tone {
            StatusTone::Neutral => self.text_muted,
            StatusTone::Running => self.primary,
            StatusTone::Success => self.success,
            StatusTone::Warning => self.warning,
            StatusTone::Danger => self.danger,
            StatusTone::Info => self.teal,
        };
        Style::default().fg(color).add_modifier(Modifier::BOLD)
    }
}
