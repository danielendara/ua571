use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use ua571_core::{AppState, MenuSection};

use crate::theme::ConsoleTheme;

pub fn draw(frame: &mut Frame, state: &AppState, theme: &ConsoleTheme, area: Rect) {
    let options = state.options();

    // Top row: SYSTEM / WEAPON / IFF / TEST
    // Bottom row: TARGET / SPECTRAL / SELECT
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let top_sections = [
        MenuSection::SystemMode,
        MenuSection::WeaponStatus,
        MenuSection::IffStatus,
        MenuSection::TestRoutine,
    ];
    let bottom_sections = [
        MenuSection::TargetProfile,
        MenuSection::SpectralProfile,
        MenuSection::TargetSelect,
    ];

    draw_section_row(frame, options, theme, rows[0], &top_sections);
    draw_section_row(frame, options, theme, rows[1], &bottom_sections);
}

fn draw_section_row(
    frame: &mut Frame,
    options: &ua571_core::OptionsState,
    theme: &ConsoleTheme,
    area: Rect,
    sections: &[MenuSection],
) {
    let constraints: Vec<Constraint> = sections
        .iter()
        .map(|_| Constraint::Ratio(1, sections.len() as u32))
        .collect();

    let cells = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    for (i, section) in sections.iter().enumerate() {
        draw_section(frame, options, theme, cells[i], *section);
    }
}

fn draw_section(
    frame: &mut Frame,
    options: &ua571_core::OptionsState,
    theme: &ConsoleTheme,
    area: Rect,
    section: MenuSection,
) {
    let focused = options.focus == section;
    let (labels, selected) = options.section_options(section);

    let border_style = if focused {
        theme.accent_style()
    } else {
        theme.border()
    };

    let title = if focused {
        format!("▶ {} ", section.label())
    } else {
        format!(" {} ", section.label())
    };

    let mut lines = Vec::new();
    for (idx, label) in labels.iter().enumerate() {
        let is_sel = idx == selected;
        let style = if is_sel && focused {
            theme.inverted()
        } else if is_sel {
            theme.accent_style()
        } else {
            theme.dim_style()
        };
        let marker = if is_sel { "►" } else { " " };
        lines.push(Line::from(Span::styled(
            format!(" {marker} {label} "),
            style,
        )));
    }

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(theme.base());

    frame.render_widget(Paragraph::new(lines).block(block), area);
}
