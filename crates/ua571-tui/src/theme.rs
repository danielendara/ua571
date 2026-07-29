//! Color themes for the operator console.

use ratatui::style::{Color, Modifier, Style};
use ua571_core::Theme;

#[derive(Debug, Clone, Copy)]
pub struct ConsoleTheme {
    pub kind: Theme,
    pub fg: Color,
    pub dim: Color,
    pub bg: Color,
    pub accent: Color,
    pub alert: Color,
    pub selected_bg: Color,
    pub selected_fg: Color,
}

impl ConsoleTheme {
    pub fn from_kind(kind: Theme) -> Self {
        match kind {
            Theme::Phosphor => Self {
                kind,
                fg: Color::Rgb(80, 250, 123),
                dim: Color::Rgb(40, 120, 60),
                bg: Color::Black,
                accent: Color::Rgb(160, 255, 180),
                alert: Color::Rgb(255, 80, 80),
                selected_bg: Color::Rgb(80, 250, 123),
                selected_fg: Color::Black,
            },
            Theme::Amber => Self {
                kind,
                fg: Color::Rgb(255, 176, 0),
                dim: Color::Rgb(140, 90, 0),
                bg: Color::Black,
                accent: Color::Rgb(255, 220, 120),
                alert: Color::Rgb(255, 60, 40),
                selected_bg: Color::Rgb(255, 176, 0),
                selected_fg: Color::Black,
            },
            Theme::Mono => Self {
                kind,
                fg: Color::White,
                dim: Color::DarkGray,
                bg: Color::Black,
                accent: Color::Gray,
                alert: Color::LightRed,
                selected_bg: Color::White,
                selected_fg: Color::Black,
            },
        }
    }

    pub fn base(&self) -> Style {
        Style::default().fg(self.fg).bg(self.bg)
    }

    pub fn dim_style(&self) -> Style {
        Style::default().fg(self.dim).bg(self.bg)
    }

    pub fn accent_style(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .bg(self.bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn alert_style(&self) -> Style {
        Style::default()
            .fg(self.alert)
            .bg(self.bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn inverted(&self) -> Style {
        Style::default()
            .fg(self.selected_fg)
            .bg(self.selected_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn border(&self) -> Style {
        Style::default().fg(self.fg).bg(self.bg)
    }

    pub fn title(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .bg(self.bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn next(self) -> Self {
        let kind = match self.kind {
            Theme::Phosphor => Theme::Amber,
            Theme::Amber => Theme::Mono,
            Theme::Mono => Theme::Phosphor,
        };
        Self::from_kind(kind)
    }
}
