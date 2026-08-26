//! `AnnotAgent` Ratatui theme derived from the canonical visual-system tokens.

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
    bg: Color,
    surface: Color,
    surface_muted: Color,
    text: Color,
    text_muted: Color,
    border: Color,
    primary: Color,
    selection: Color,
    teal: Color,
    success: Color,
    warning: Color,
    danger: Color,
}

impl AnnotAgentTheme {
    #[must_use]
    pub fn detect() -> Self {
        let true_color = std::env::var("COLORTERM").is_ok_and(|value| {
            matches!(value.to_ascii_lowercase().as_str(), "truecolor" | "24bit")
        });
        if true_color {
            Self::true_color()
        } else {
            Self::ansi256()
        }
    }

    #[must_use]
    pub const fn true_color() -> Self {
        Self {
            bg: Color::Rgb(7, 17, 31),
            surface: Color::Rgb(11, 23, 42),
            surface_muted: Color::Rgb(18, 36, 58),
            text: Color::Rgb(248, 250, 252),
            text_muted: Color::Rgb(148, 163, 184),
            border: Color::Rgb(38, 58, 82),
            primary: Color::Rgb(96, 165, 250),
            selection: Color::Rgb(37, 99, 235),
            teal: Color::Rgb(45, 212, 191),
            success: Color::Rgb(74, 222, 128),
            warning: Color::Rgb(251, 191, 36),
            danger: Color::Rgb(248, 113, 113),
        }
    }

    #[must_use]
    pub const fn ansi256() -> Self {
        Self {
            bg: Color::Indexed(233),
            surface: Color::Indexed(234),
            surface_muted: Color::Indexed(235),
            text: Color::Indexed(255),
            text_muted: Color::Indexed(246),
            border: Color::Indexed(238),
            primary: Color::Indexed(75),
            selection: Color::Indexed(27),
            teal: Color::Indexed(43),
            success: Color::Indexed(77),
            warning: Color::Indexed(214),
            danger: Color::Indexed(203),
        }
    }

    #[must_use]
    pub fn base(self) -> Style {
        Style::default().fg(self.text).bg(self.bg)
    }

    #[must_use]
    pub fn panel(self) -> Style {
        Style::default().fg(self.text).bg(self.surface)
    }

    #[must_use]
    pub fn panel_muted(self) -> Style {
        Style::default().fg(self.text).bg(self.surface_muted)
    }

    #[must_use]
    pub fn border(self) -> Style {
        Style::default().fg(self.border).bg(self.surface)
    }

    #[must_use]
    pub fn selected(self) -> Style {
        Style::default()
            .fg(Color::White)
            .bg(self.selection)
            .add_modifier(Modifier::BOLD)
    }

    #[must_use]
    pub fn title(self) -> Style {
        Style::default()
            .fg(self.teal)
            .bg(self.bg)
            .add_modifier(Modifier::BOLD)
    }

    #[must_use]
    pub fn muted(self) -> Style {
        Style::default().fg(self.text_muted).bg(self.bg)
    }

    #[must_use]
    pub fn command(self) -> Style {
        Style::default().fg(self.primary).bg(self.surface_muted)
    }

    #[must_use]
    pub fn status(self, tone: StatusTone) -> Style {
        let color = match tone {
            StatusTone::Neutral => self.text_muted,
            StatusTone::Running => self.primary,
            StatusTone::Success => self.success,
            StatusTone::Warning => self.warning,
            StatusTone::Danger => self.danger,
            StatusTone::Info => self.teal,
        };
        Style::default()
            .fg(color)
            .bg(self.surface)
            .add_modifier(Modifier::BOLD)
    }
}
