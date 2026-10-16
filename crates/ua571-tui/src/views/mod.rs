pub mod boot;
pub mod fire;
pub mod options;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use ua571_core::{sentry_mark_char, AppState, Screen};

use crate::theme::ConsoleTheme;

/// Smallest terminal the Fire/Options console renders without clipping the
/// header, sentry bar, or start of the help row. Deliberately below the
/// ideal ~26 rows so a standard 80×24 terminal still gets the console.
pub const MIN_COLS: u16 = 80;
pub const MIN_ROWS: u16 = 20;

/// Render the console. `fire_status_hint` is a one-shot fire-deny/critical
/// suffix (CRITICAL / EMPTY / LINK DOWN / OFFLINE) for the sentry bar — the
/// TUI's analog of web's `#status` hint and pixel's window-title hint.
/// `None` for no recent fire attempt, or an ordinary
/// successful non-critical fire.
pub fn draw(
    frame: &mut Frame,
    state: &AppState,
    theme: &ConsoleTheme,
    fire_status_hint: Option<&str>,
) {
    let area = frame.area();
    frame.render_widget(Block::default().style(theme.base()), area);

    match state.screen {
        Screen::Boot => {
            boot::draw(frame, state, theme, area);
            return;
        }
        Screen::Options | Screen::Fire => {}
    }

    if area.width < MIN_COLS || area.height < MIN_ROWS {
        draw_too_small(frame, theme, area);
        return;
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
    draw_sentry_bar(frame, state, theme, chunks[1], fire_status_hint);

    match state.screen {
        Screen::Options => options::draw(frame, state, theme, chunks[2]),
        Screen::Fire => fire::draw(frame, state, theme, chunks[2]),
        Screen::Boot => unreachable!(),
    }

    draw_log(frame, state, theme, chunks[3]);
    draw_help(frame, state, theme, chunks[4]);
}

fn too_small_text(width: u16, height: u16) -> String {
    format!(
        "TERMINAL TOO SMALL — resize to at least {MIN_COLS}×{MIN_ROWS} (now {width}×{height}) · q quit"
    )
}

fn draw_too_small(frame: &mut Frame, theme: &ConsoleTheme, area: Rect) {
    let text = too_small_text(area.width, area.height);
    let rows = (text.chars().count() as u16)
        .div_ceil(area.width.max(1))
        .min(area.height);
    let centred = Rect {
        y: area.y + (area.height - rows) / 2,
        height: rows,
        ..area
    };
    frame.render_widget(
        Paragraph::new(text)
            .style(theme.base())
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(ratatui::widgets::Wrap { trim: true }),
        centred,
    );
}

fn draw_header(frame: &mut Frame, state: &AppState, theme: &ConsoleTheme, area: Rect) {
    let demo = if state.demo.is_active() {
        Span::styled(" DEMO ", theme.inverted())
    } else {
        Span::styled(" demo:off ", theme.dim_style())
    };

    let screen = Span::styled(format!(" {} ", state.screen.label()), theme.accent_style());

    // Sentry 1→A … 4→D (matches pixel/web circled unit marks).
    let mark = sentry_mark_char(state.active_sentry().id);
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

fn draw_sentry_bar(
    frame: &mut Frame,
    state: &AppState,
    theme: &ConsoleTheme,
    area: Rect,
    fire_status_hint: Option<&str>,
) {
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

    // One-shot fire-status suffix (CRITICAL / EMPTY / LINK DOWN / OFFLINE),
    // matching web's ` · HINT` chrome suffix and pixel's title suffix.
    if let Some(hint) = fire_status_hint {
        spans.push(Span::styled(
            format!(" │ FIRE: {hint}"),
            theme.alert_style(),
        ));
    }

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

/// Help-row text per screen. Fire and Options both mention `esc toggle`
/// (Esc already toggles Fire ↔ Options via the shared `Command::ToggleFirePanel` —
/// see `ua571-core::keys::apply_command` — this only adds discoverability, no binding
/// change), matching web's footer legend and pixel's HUD line
/// (`ua571-render::draw`) in spirit, not verbatim wording/casing — TUI's
/// existing labels are already lowercase and terser than either. Boot skips
/// POST via any key rather than routing through `Command::ToggleFirePanel`,
/// so its line is unchanged: mentioning "toggle" there would be inaccurate.
fn help_text(screen: Screen) -> &'static str {
    match screen {
        Screen::Fire => {
            "Enter/Space fire  o options  esc toggle  a arm  r reload  1-4 sentry  d demo  m sound  t theme  q quit"
        }
        Screen::Options => {
            "←→ section  ↑↓ select  f fire  esc toggle  a arm  1-4 sentry  d demo  m sound  t theme  q quit"
        }
        Screen::Boot => {
            "←→ section  ↑↓ select  f fire  a arm  1-4 sentry  d demo  m sound  t theme  q quit"
        }
    }
}

fn draw_help(frame: &mut Frame, state: &AppState, theme: &ConsoleTheme, area: Rect) {
    let text = help_text(state.screen);
    frame.render_widget(Paragraph::new(Span::styled(text, theme.dim_style())), area);
}

#[cfg(test)]
mod help_text_tests {
    use super::*;

    /// Esc already toggles Fire ↔ Options (`ua571-core::keys::Command::ToggleFirePanel`)
    /// — this only checks the help row *mentions* it, not the binding itself.
    #[test]
    fn fire_and_options_help_mention_esc_toggle() {
        for screen in [Screen::Fire, Screen::Options] {
            let text = help_text(screen).to_ascii_lowercase();
            assert!(
                text.contains("esc"),
                "{screen:?} help should mention Esc: {text:?}"
            );
            assert!(
                text.contains("toggle"),
                "{screen:?} help should say Esc toggles: {text:?}"
            );
        }
    }

    #[test]
    fn boot_help_is_unchanged_and_does_not_claim_a_toggle() {
        // Boot skips POST on any key rather than routing through
        // Command::ToggleFirePanel's Fire/Options toggle, so it shouldn't claim one.
        assert!(!help_text(Screen::Boot)
            .to_ascii_lowercase()
            .contains("toggle"));
    }

    #[test]
    fn tui_help_and_web_footer_share_command_tokens() {
        const TOKENS: &[&str] = &[
            "fire", "options", "esc", "arm", "reload", "sentry", "demo", "sound", "theme",
        ];
        let tui = format!("{} {}", help_text(Screen::Fire), help_text(Screen::Options))
            .to_ascii_lowercase();
        let html = include_str!("../../../../web/index.html");
        let keys_start = html
            .find("<p class=\"keys\">")
            .expect("web footer keys block");
        let keys_end = html[keys_start..]
            .find("</p>")
            .expect("web footer keys close");
        let web = html[keys_start..keys_start + keys_end].to_ascii_lowercase();
        for token in TOKENS {
            assert!(tui.contains(token), "TUI help missing {token:?}: {tui:?}");
            assert!(web.contains(token), "web footer missing {token:?}: {web:?}");
        }
    }

    #[test]
    fn help_text_fits_a_120_column_terminal() {
        for screen in [Screen::Fire, Screen::Options, Screen::Boot] {
            let text = help_text(screen);
            assert!(
                text.chars().count() <= 120,
                "{screen:?} help text is {} chars, expected <= 120: {text:?}",
                text.chars().count(),
            );
        }
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use ua571_core::Config;

    fn render(screen: Screen, w: u16, h: u16) -> String {
        let mut state = AppState::new(Config::default());
        state.skip_boot();
        state.set_screen(screen);
        let theme = ConsoleTheme::from_kind(state.config.theme);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, &state, &theme, None)).unwrap();
        let buf = term.backend().buffer();
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn small_terminal_shows_notice_only() {
        for screen in [Screen::Fire, Screen::Options] {
            let out = render(screen, 60, 18);
            assert!(out.contains("TERMINAL TOO SMALL"), "{out}");
            assert!(out.contains("60×18"), "{out}");
            assert!(!out.contains("REMOTE SENTRY"), "{out}");
            assert!(!out.contains("esc toggle"), "{out}");
        }
        assert!(render(Screen::Fire, MIN_COLS - 1, 30).contains("TERMINAL TOO SMALL"));
        assert!(render(Screen::Fire, 120, MIN_ROWS - 1).contains("TERMINAL TOO SMALL"));
    }

    #[test]
    fn standard_80x24_renders_console() {
        for screen in [Screen::Fire, Screen::Options] {
            let out = render(screen, 80, 24);
            assert!(!out.contains("TERMINAL TOO SMALL"), "{out}");
            assert!(out.contains("REMOTE SENTRY"), "{out}");
        }
    }

    #[test]
    fn min_size_renders_header_sentry_bar_and_help_start() {
        for screen in [Screen::Fire, Screen::Options] {
            let out = render(screen, MIN_COLS, MIN_ROWS);
            assert!(!out.contains("TERMINAL TOO SMALL"), "{out}");
            assert!(out.contains("REMOTE SENTRY"), "{out}");
            assert!(out.contains("S1:"), "{out}");
            let start: String = help_text(screen).chars().take(12).collect();
            assert!(out.contains(&start), "{out}");
        }
    }

    #[test]
    fn boot_screen_ignores_size_guard() {
        let state = AppState::new(Config::default());
        let theme = ConsoleTheme::from_kind(state.config.theme);
        let mut term = Terminal::new(TestBackend::new(40, 10)).unwrap();
        term.draw(|f| draw(f, &state, &theme, None)).unwrap();
        let buf = format!("{:?}", term.backend().buffer());
        assert!(!buf.contains("TERMINAL TOO SMALL"));
    }
}
