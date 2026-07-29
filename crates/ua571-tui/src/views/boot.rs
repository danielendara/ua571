use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use ua571_core::AppState;

use crate::theme::ConsoleTheme;

pub fn draw(frame: &mut Frame, state: &AppState, theme: &ConsoleTheme, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Length(12),
            Constraint::Min(1),
        ])
        .split(area);

    let progress = if state.boot_ticks_remaining > 0 {
        let total = 40u32;
        let done = total.saturating_sub(state.boot_ticks_remaining);
        let filled = ((done as f32 / total as f32) * 24.0) as usize;
        let bar = format!(
            "[{}{}]",
            "█".repeat(filled.min(24)),
            "░".repeat(24usize.saturating_sub(filled))
        );
        bar
    } else {
        "[████████████████████████]".into()
    };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "W-Y COMBINED SYSTEMS  //  USCM  //  HYPERDYNE",
            theme.dim_style(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "UA 571-C  REMOTE SENTRY WEAPON SYSTEM",
            theme.title(),
        )),
        Line::from(Span::styled("OPERATOR CONSOLE  v0.1", theme.accent_style())),
        Line::from(""),
        Line::from(Span::styled("POST — MICROWAVE DATALINK", theme.base())),
        Line::from(Span::styled(
            "  SENTRY-1..4  LINK CHECK .... OK",
            theme.dim_style(),
        )),
        Line::from(Span::styled(
            "  AMMUNITION DRUM SENSORS ..... OK",
            theme.dim_style(),
        )),
        Line::from(Span::styled(format!("  INIT  {progress}"), theme.base())),
        Line::from(""),
        Line::from(Span::styled("press any key to skip", theme.dim_style())),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border())
        .title(" SYSTEM BOOT ")
        .style(theme.base());

    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(block),
        chunks[1],
    );
}
