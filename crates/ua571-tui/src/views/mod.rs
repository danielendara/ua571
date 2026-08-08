pub mod boot;
pub mod fire;
pub mod options;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use ua571_core::{AppState, Screen};

use crate::theme::ConsoleTheme;

pub fn draw(frame: &mut Frame, state: &AppState, theme: &ConsoleTheme) {
    let area = frame.area();
    frame.render_widget(Block::default().style(theme.base()), area);

    match state.screen {
        Screen::Boot => {
            boot::draw(frame, state, theme, area);
            return;
        }
        Screen::Options | Screen::Fire => {}
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Length(1), // sentry bar
            Constraint::Min(8),    // main
            Constraint::Length(5), // log
            Constraint::Length(1), // help
        ])
        .split(area);

    draw_header(frame, state, theme, chunks[0]);
    draw_sentry_bar(frame, state, theme, chunks[1]);

    match state.screen {
        Screen::Options => options::draw(frame, state, theme, chunks[2]),
        Screen::Fire => fire::draw(frame, state, theme, chunks[2]),
        Screen::Boot => unreachable!(),
    }

    draw_log(frame, state, theme, chunks[3]);
    draw_help(frame, state, theme, chunks[4]);
}

fn draw_header(frame: &mut Frame, state: &AppState, theme: &ConsoleTheme, area: Rect) {
    let demo = if state.demo.is_active() {
        Span::styled(" DEMO ", theme.inverted())
    } else {
        Span::styled(" demo:off ", theme.dim_style())
    };

    let screen = Span::styled(format!(" {} ", state.screen.label()), theme.accent_style());

    // Sentry 1→A … 4→D (matches pixel/web circled unit marks).
    let mark = match state.active_sentry().id {
        1 => 'A',
        2 => 'B',
        3 => 'C',
        4 => 'D',
        _ => 'A',
    };
    let mark_l = format!(" ({mark}) ");
    let mark_r = format!("({mark}) ");

    let title = Line::from(vec![
        Span::styled(mark_l, theme.accent_style()),
        Span::styled("UA 571-C", theme.title().add_modifier(Modifier::BOLD)),
        Span::styled("  REMOTE SENTRY WEAPON SYSTEM  ", theme.base()),
        Span::styled(mark_r, theme.accent_style()),
        screen,
        demo,
        Span::styled(format!("  [{}] ", theme.kind.as_str()), theme.dim_style()),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border())
        .style(theme.base());

    frame.render_widget(Paragraph::new(title).block(block), area);
}

fn draw_sentry_bar(frame: &mut Frame, state: &AppState, theme: &ConsoleTheme, area: Rect) {
    let mut spans = Vec::new();
    for (i, s) in state.bank.iter().enumerate() {
        let label = format!(
            " S{}:{} {}rds ",
            s.id,
            if s.is_armed() { "ARMED" } else { "SAFE" },
            s.fire.rounds
        );
        let style = if i == state.active_index {
            theme.inverted()
        } else if !s.link_ok {
            theme.alert_style()
        } else {
            theme.dim_style()
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }

    let active = state.active_sentry();
    spans.push(Span::styled(
        format!(
            "│ {} │ {} │ LINK {}",
            active.options.system_mode.label(),
            active.options.iff_status.label(),
            if active.link_ok { "OK" } else { "DOWN" }
        ),
        theme.base(),
    ));

    frame.render_widget(Paragraph::new(Line::from(spans)).style(theme.base()), area);
}

fn draw_log(frame: &mut Frame, state: &AppState, theme: &ConsoleTheme, area: Rect) {
    let recent = state.log.recent(3);
    let lines: Vec<Line> = if recent.is_empty() {
        vec![Line::from(Span::styled("— no events —", theme.dim_style()))]
    } else {
        recent
            .into_iter()
            .map(|e| Line::from(Span::styled(format!("› {}", e.kind), theme.dim_style())))
            .collect()
    };

    let block = Block::default()
        .title(" EVENT LOG ")
        .borders(Borders::ALL)
        .border_style(theme.border())
        .style(theme.base());

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_help(frame: &mut Frame, state: &AppState, theme: &ConsoleTheme, area: Rect) {
    let text = match state.screen {
        Screen::Fire => {
            "Enter/Space fire  o options  a arm/safe  r reload  1-4 sentry  d demo  t theme  q quit"
        }
        _ => "←→ section  ↑↓ select  f fire panel  a arm/safe  1-4 sentry  d demo  t theme  q quit",
    };
    frame.render_widget(Paragraph::new(Span::styled(text, theme.dim_style())), area);
}
