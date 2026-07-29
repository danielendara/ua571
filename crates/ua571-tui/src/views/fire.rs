use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;
use ua571_core::{AppState, WeaponStatus};

use crate::theme::ConsoleTheme;

pub fn draw(frame: &mut Frame, state: &AppState, theme: &ConsoleTheme, area: Rect) {
    let sentry = state.active_sentry();
    let fire = &sentry.fire;

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(area);

    // Left: rounds + time + critical
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Min(1),
        ])
        .split(cols[0]);

    let rounds_block = Block::default()
        .title(" ROUNDS REMAINING ")
        .borders(Borders::ALL)
        .border_style(theme.border())
        .style(theme.base());
    let rounds_style = if fire.critical {
        theme.alert_style()
    } else {
        theme.title()
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("{:3}", fire.rounds),
            rounds_style,
        )))
        .alignment(Alignment::Center)
        .block(rounds_block),
        left[0],
    );

    let time_block = Block::default()
        .title(" TIME AT 100% (secs) ")
        .borders(Borders::ALL)
        .border_style(theme.border())
        .style(theme.base());
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            fire.time_display(),
            theme.accent_style(),
        )))
        .alignment(Alignment::Center)
        .block(time_block),
        left[1],
    );

    let crit_text = if fire.critical {
        if fire.critical_blink {
            "  CRITICAL  "
        } else {
            "            "
        }
    } else {
        "  NOMINAL   "
    };
    let crit_style = if fire.critical && fire.critical_blink {
        theme.inverted()
    } else if fire.critical {
        theme.alert_style()
    } else {
        theme.dim_style()
    };
    let crit_block = Block::default()
        .title(" STATUS ")
        .borders(Borders::ALL)
        .border_style(if fire.critical {
            theme.alert_style()
        } else {
            theme.border()
        })
        .style(theme.base());
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(crit_text, crit_style)))
            .alignment(Alignment::Center)
            .block(crit_block),
        left[2],
    );

    let weapon = match sentry.options.weapon_status {
        WeaponStatus::Safe => Span::styled(" SAFE ", theme.dim_style()),
        WeaponStatus::Armed => Span::styled(" ARMED ", theme.inverted()),
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {}  ", sentry.label()), theme.base()),
            weapon,
            Span::styled(
                format!(
                    "  {} / {} / {}",
                    sentry.options.system_mode.label(),
                    sentry.options.target_profile.label(),
                    sentry.options.spectral_profile.label()
                ),
                theme.dim_style(),
            ),
        ])),
        left[3],
    );

    // Vertical gauges — fill bottom → top like the original GRiD panel.
    let temp_style = if fire.temperature >= 80 {
        theme.alert_style()
    } else {
        theme.accent_style()
    };
    draw_vertical_gauge(
        frame,
        cols[1],
        theme,
        GaugeSpec {
            style: temp_style,
            title: " TEMP ",
            value: fire.temperature as u16,
            max: 100,
            label: format!("{}°", fire.temperature),
        },
    );

    draw_vertical_gauge(
        frame,
        cols[2],
        theme,
        GaugeSpec {
            style: theme.accent_style(),
            title: " R(M) ",
            value: fire.rm as u16,
            max: 100,
            label: format!("{} rpm", fire.rm),
        },
    );
}

struct GaugeSpec {
    style: Style,
    title: &'static str,
    value: u16,
    max: u16,
    label: String,
}

/// Column gauge that fills from the bottom upward (original sentry display).
fn draw_vertical_gauge(frame: &mut Frame, area: Rect, theme: &ConsoleTheme, spec: GaugeSpec) {
    let ratio = if spec.max == 0 {
        0.0
    } else {
        (spec.value as f64 / spec.max as f64).clamp(0.0, 1.0)
    };

    let block = Block::default()
        .title(spec.title)
        .borders(Borders::ALL)
        .border_style(theme.border())
        .style(theme.base());

    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Top row: numeric readout; remaining rows are the column.
    let (label_area, bar_area) = if inner.height >= 2 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner);
        (chunks[0], chunks[1])
    } else {
        (Rect::default(), inner)
    };

    if label_area.height > 0 {
        Paragraph::new(Line::from(Span::styled(spec.label, spec.style)))
            .alignment(Alignment::Center)
            .render(label_area, frame.buffer_mut());
    }

    if bar_area.height == 0 || bar_area.width == 0 {
        return;
    }

    // Center a relatively thin bar column (original gauges were narrow).
    let bar_width = bar_area.width.clamp(3, 8).min(bar_area.width);
    let bar_x = bar_area.x + (bar_area.width.saturating_sub(bar_width)) / 2;
    let bar_rect = Rect {
        x: bar_x,
        y: bar_area.y,
        width: bar_width,
        height: bar_area.height,
    };

    let filled_rows = ((f64::from(bar_rect.height) * ratio).round() as u16).min(bar_rect.height);
    let empty_style = theme.dim_style();
    let filled_style = spec.style;
    let buf = frame.buffer_mut();

    for row in 0..bar_rect.height {
        // row 0 is the top of the column; fill grows from the bottom.
        let from_bottom = bar_rect.height - 1 - row;
        let filled = from_bottom < filled_rows;
        let y = bar_rect.y + row;

        // Side tick marks (every other row) like the original scale lines.
        let tick = row % 2 == 0;
        if bar_rect.x > bar_area.x {
            let ch = if tick { '┤' } else { '│' };
            buf[(bar_rect.x - 1, y)].set_char(ch).set_style(empty_style);
        }
        let right = bar_rect.x + bar_rect.width;
        if right < bar_area.x + bar_area.width {
            let ch = if tick { '├' } else { '│' };
            buf[(right, y)].set_char(ch).set_style(empty_style);
        }

        let ch = if filled { '█' } else { '░' };
        let style = if filled { filled_style } else { empty_style };
        for dx in 0..bar_rect.width {
            buf[(bar_rect.x + dx, y)].set_char(ch).set_style(style);
        }
    }
}
