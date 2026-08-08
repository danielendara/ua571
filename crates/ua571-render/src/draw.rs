//! Scene drawing ported from the GRiD Pascal UA571C layout coordinates.

use ua571_core::{AppState, MenuSection, Screen, WeaponStatus};

use crate::fb::{Framebuffer, HEIGHT, WIDTH};

const SECTION_SCALE: i32 = 1;
const HEADER_SCALE: i32 = 2;
const BIG_SCALE: i32 = 3;

pub fn render(state: &AppState, fb: &mut Framebuffer) {
    fb.clear();
    match state.screen {
        Screen::Boot => draw_boot(state, fb),
        Screen::Options => {
            draw_header(state, fb);
            draw_options(state, fb);
            draw_status_strip(state, fb);
        }
        Screen::Fire => {
            draw_header(state, fb);
            draw_fire(state, fb);
            draw_status_strip(state, fb);
        }
    }
}

/// Sentry 1→A … 4→D (circled unit marks in the header).
fn sentry_mark(id: u8) -> char {
    match id {
        1 => 'A',
        2 => 'B',
        3 => 'C',
        4 => 'D',
        _ => 'A',
    }
}

/// HEADER.PAS DisplayHeader + circled unit letter for the active sentry.
fn draw_header(state: &AppState, fb: &mut Framebuffer) {
    // Title centered-ish at original positions (scaled fonts approximate Tb12x16 / Tb24x32).
    let title = "UA 571-C";
    let tw = Framebuffer::text_width(title, HEADER_SCALE);
    fb.draw_text(title, (WIDTH as i32 - tw) / 2, 2, HEADER_SCALE);

    let sub = "REMOTE SENTRY WEAPON SYSTEM";
    let sw = Framebuffer::text_width(sub, SECTION_SCALE);
    fb.draw_text(sub, (WIDTH as i32 - sw) / 2, 20, SECTION_SCALE);

    // Circled unit marks — letter tracks active sentry (1=A … 4=D).
    let mark = sentry_mark(state.active_sentry().id);
    draw_circled_letter(fb, mark, 18, 17);
    draw_circled_letter(fb, mark, WIDTH as i32 - 18, 17);

    fb.line(0, 35, WIDTH as i32 - 1, 35);
}

/// Draw `ch` centered inside a circle at (`cx`, `cy`).
fn draw_circled_letter(fb: &mut Framebuffer, ch: char, cx: i32, cy: i32) {
    let scale = 2;
    // 8×8 glyph at scale 2 → 16×16 box; center on the circle midpoint.
    let glyph = 8 * scale;
    let gx = cx - glyph / 2;
    let gy = cy - glyph / 2;
    fb.draw_char(ch, gx, gy, scale);
    fb.circle(cx, cy, 14);
}

/// MAINDISP.PAS DrawDisplay + selection invert rects.
fn draw_options(state: &AppState, fb: &mut Framebuffer) {
    let opts = state.options();

    // Column headers (approx original y=39/52).
    fb.draw_text("SYSTEM", 36, 39, SECTION_SCALE);
    fb.draw_text("MODE", 45, 50, SECTION_SCALE);
    fb.draw_text("WEAPON", 210, 39, SECTION_SCALE);
    fb.draw_text("STATUS", 210, 50, SECTION_SCALE);
    fb.draw_text("IFF", 382, 39, SECTION_SCALE);
    fb.draw_text("STATUS", 364, 50, SECTION_SCALE);
    fb.draw_text("TEST", 536, 39, SECTION_SCALE);
    fb.draw_text("ROUTINE", 518, 50, SECTION_SCALE);

    fb.line(0, 66, WIDTH as i32 - 1, 66);
    fb.line(160, 35, 160, 66);
    fb.line(320, 35, 320, 66);
    fb.line(480, 35, 480, 66);
    fb.line(160, 66, 160, 124);
    fb.line(320, 66, 320, 124);
    fb.line(480, 66, 480, 124);

    // Options text (original spacing).
    fb.draw_text("AUTO-REMOTE", 14, 69, SECTION_SCALE);
    fb.draw_text("MAN-OVERRIDE", 14, 83, SECTION_SCALE);
    fb.draw_text("SEMI-AUTO", 14, 97, SECTION_SCALE);

    fb.draw_text("SAFE", 216, 69, SECTION_SCALE);
    fb.draw_text("ARMED", 216, 83, SECTION_SCALE);

    fb.draw_text("SEARCH", 348, 69, SECTION_SCALE);
    fb.draw_text("TEST", 348, 83, SECTION_SCALE);
    fb.draw_text("ENGAGED", 348, 97, SECTION_SCALE);
    fb.draw_text("INTERROGATE", 348, 111, SECTION_SCALE);

    fb.draw_text("AUTO", 504, 69, SECTION_SCALE);
    fb.draw_text("SELECTIVE", 504, 83, SECTION_SCALE);

    fb.line(0, 124, WIDTH as i32 - 1, 124);
    fb.line(213, 124, 213, 142);
    fb.line(427, 124, 427, 142);
    fb.line(0, 142, WIDTH as i32 - 1, 142);

    fb.draw_text("TARGET PROFILE", 40, 128, SECTION_SCALE);
    fb.draw_text("SPECTRAL PROFILE", 240, 128, SECTION_SCALE);
    fb.draw_text("TARGET SELECT", 466, 128, SECTION_SCALE);

    fb.draw_text("SOFT", 72, 145, SECTION_SCALE);
    fb.draw_text("SEMI", 72, 159, SECTION_SCALE);
    fb.draw_text("HARD", 72, 173, SECTION_SCALE);

    fb.draw_text("BIO", 291, 145, SECTION_SCALE);
    fb.draw_text("INERT", 291, 159, SECTION_SCALE);

    fb.draw_text("MULTI SPEC", 483, 145, SECTION_SCALE);
    fb.draw_text("INFRA RED", 483, 159, SECTION_SCALE);
    fb.draw_text("UV", 483, 173, SECTION_SCALE);

    fb.line(0, 212, WIDTH as i32 - 1, 212);
    fb.line(213, 142, 213, 212);
    fb.line(427, 142, 427, 212);

    // Focus section header invert (menu strip).
    if let Some((x, y, w, h)) = menu_section_rect(opts.focus) {
        fb.invert_rect(x, y, w, h);
    }

    // Selected option row invert within each column.
    invert_selection(fb, MenuSection::SystemMode, opts.system_mode.index());
    invert_selection(fb, MenuSection::WeaponStatus, opts.weapon_status.index());
    invert_selection(fb, MenuSection::IffStatus, opts.iff_status.index());
    invert_selection(fb, MenuSection::TestRoutine, opts.test_routine.index());
    invert_selection(fb, MenuSection::TargetProfile, opts.target_profile.index());
    invert_selection(
        fb,
        MenuSection::SpectralProfile,
        opts.spectral_profile.index(),
    );
    invert_selection(fb, MenuSection::TargetSelect, opts.target_select.index());
}

fn menu_section_rect(section: MenuSection) -> Option<(i32, i32, i32, i32)> {
    // From SetMenuRectangle in MAINDISP.PAS
    Some(match section {
        MenuSection::SystemMode => (1, 37, 158, 28),
        MenuSection::WeaponStatus => (162, 37, 157, 28),
        MenuSection::IffStatus => (322, 37, 157, 28),
        MenuSection::TestRoutine => (482, 37, 155, 28),
        MenuSection::TargetProfile => (1, 126, 211, 15),
        MenuSection::SpectralProfile => (215, 126, 211, 15),
        MenuSection::TargetSelect => (429, 126, 208, 15),
    })
}

fn invert_selection(fb: &mut Framebuffer, section: MenuSection, index: usize) {
    let rect = match section {
        MenuSection::SystemMode => match index {
            0 => (1, 68, 158, 13),
            1 => (1, 82, 158, 13),
            2 => (1, 96, 157, 13),
            _ => return,
        },
        MenuSection::WeaponStatus => match index {
            0 => (162, 68, 157, 13),
            1 => (162, 82, 157, 13),
            _ => return,
        },
        MenuSection::IffStatus => match index {
            0 => (322, 68, 157, 13),
            1 => (322, 82, 157, 13),
            2 => (322, 96, 157, 13),
            3 => (322, 110, 157, 13),
            _ => return,
        },
        MenuSection::TestRoutine => match index {
            0 => (482, 68, 155, 13),
            1 => (482, 82, 155, 13),
            _ => return,
        },
        MenuSection::TargetProfile => match index {
            0 => (1, 144, 211, 13),
            1 => (1, 158, 211, 13),
            2 => (1, 172, 211, 13),
            _ => return,
        },
        MenuSection::SpectralProfile => match index {
            0 => (215, 144, 211, 13),
            1 => (215, 158, 211, 13),
            _ => return,
        },
        MenuSection::TargetSelect => match index {
            0 => (429, 144, 208, 13),
            1 => (429, 158, 208, 13),
            2 => (429, 172, 208, 13),
            _ => return,
        },
    };
    fb.invert_rect(rect.0, rect.1, rect.2, rect.3);
}

/// FIRE.PAS DrawDisplay + live gauges / CRITICAL.
fn draw_fire(state: &AppState, fb: &mut Framebuffer) {
    let s = state.active_sentry();
    let fire = &s.fire;

    fb.draw_text("ROUNDS", 20, 70, SECTION_SCALE);
    fb.draw_text("REMAINING", 20, 84, SECTION_SCALE);

    // Boxes sized to our bitmap font (HEADER_SCALE), not original Tb12x16 pixel boxes.
    let rounds = format!("{}", fire.rounds);
    draw_boxed_value(fb, &rounds, 170, 68, HEADER_SCALE, 10, 8);

    fb.draw_text("TIME AT 100%", 6, 150, SECTION_SCALE);
    fb.draw_text("(SECS)", 30, 168, SECTION_SCALE);
    draw_boxed_value(fb, &fire.time_display(), 160, 144, HEADER_SCALE, 10, 8);

    fb.draw_text("Temp   R(M)", 500, 50, SECTION_SCALE);

    // Gauge frames (original vertical bar geometry).
    // Temp gauge around x=508..528, R(M) around 576..596, y 68..180.
    draw_gauge_frame(fb, 508, 68, 180);
    draw_gauge_frame(fb, 576, 68, 180);

    // Fill from bottom: temp and rm as 0..=100 → y from 180 up to 68 (112px span).
    fill_gauge_bottom_up(fb, 508, 68, 180, 20, fire.temperature as f64 / 100.0);
    fill_gauge_bottom_up(fb, 576, 68, 180, 20, fire.rm as f64 / 100.0);

    // CRITICAL triple-border invert flash (ShowCritical / ToggleCritical).
    if fire.critical {
        fb.draw_text("CRITICAL", 25, 118, SECTION_SCALE);
        // Outer box
        fb.rect_outline(6, 101, 106, 44);
        fb.rect_outline(12, 107, 94, 32);
        fb.rect_outline(18, 113, 82, 20);
        if fire.critical_blink {
            fb.invert_rect(6, 101, 106, 44);
        }
    }

    // Weapon status cue under gauges
    let armed = match s.options.weapon_status {
        WeaponStatus::Safe => "SAFE",
        WeaponStatus::Armed => "ARMED",
    };
    fb.draw_text(&format!("{}  {}", s.label(), armed), 20, 200, SECTION_SCALE);
}

/// Draw `text` centered inside a rect with padding (fixes overflow of fixed GRiD boxes).
fn draw_boxed_value(
    fb: &mut Framebuffer,
    text: &str,
    box_x: i32,
    box_y: i32,
    scale: i32,
    pad_x: i32,
    pad_y: i32,
) {
    let scale = scale.max(1);
    let tw = Framebuffer::text_width(text, scale);
    let th = 8 * scale;
    // Minimum width so single/double digit values still look like a console box.
    let inner_w = tw.max(3 * 8 * scale);
    let w = inner_w + pad_x * 2;
    let h = th + pad_y * 2;
    fb.rect_outline(box_x, box_y, w, h);
    let tx = box_x + (w - tw) / 2;
    let ty = box_y + (h - th) / 2;
    fb.draw_text(text, tx, ty, scale);
}

fn draw_gauge_frame(fb: &mut Framebuffer, x_left: i32, y_top: i32, y_bot: i32) {
    // Match original tick marks.
    fb.line(x_left + 12, y_top, x_left + 20, y_top);
    fb.line(x_left + 20, y_top, x_left + 20, y_bot);
    fb.line(x_left, y_bot, x_left + 20, y_bot);

    let mut y = y_bot;
    while y > y_top {
        y -= 16;
        if y >= y_top {
            fb.line(x_left + 12, y, x_left + 20, y);
        }
    }
    y = y_bot - 8;
    while y > y_top {
        fb.line(x_left + 16, y, x_left + 20, y);
        y -= 16;
    }
}

fn fill_gauge_bottom_up(
    fb: &mut Framebuffer,
    x: i32,
    y_top: i32,
    y_bot: i32,
    width: i32,
    ratio: f64,
) {
    let ratio = ratio.clamp(0.0, 1.0);
    let span = y_bot - y_top;
    if span <= 0 {
        return;
    }
    let filled = ((span as f64) * ratio).round() as i32;
    if filled <= 0 {
        return;
    }
    let y0 = y_bot - filled;
    fb.fill_rect(x, y0, width, filled, true);
}

fn draw_status_strip(state: &AppState, fb: &mut Framebuffer) {
    // Thin footer below original content.
    let y = 218;
    fb.line(0, y - 2, WIDTH as i32 - 1, y - 2);
    let s = state.active_sentry();
    let demo = if state.demo.is_active() { "DEMO" } else { "" };
    let line = format!(
        "S{}  {}rds  {}  {}  {}  [1-4] f/o fire  a arm  d demo  q quit  {}",
        s.id,
        s.fire.rounds,
        s.options.system_mode.label(),
        s.options.iff_status.label(),
        if s.is_armed() { "ARMED" } else { "SAFE" },
        demo
    );
    fb.draw_text(&line, 4, y, SECTION_SCALE);
}

fn draw_boot(state: &AppState, fb: &mut Framebuffer) {
    let cx = WIDTH as i32 / 2;
    fb.draw_text("W-Y COMBINED SYSTEMS", cx - 80, 40, SECTION_SCALE);
    fb.draw_text("USCM / HYPERDYNE", cx - 64, 54, SECTION_SCALE);

    let title = "UA 571-C";
    let tw = Framebuffer::text_width(title, BIG_SCALE);
    fb.draw_text(title, (WIDTH as i32 - tw) / 2, 80, BIG_SCALE);

    let sub = "REMOTE SENTRY WEAPON SYSTEM";
    let sw = Framebuffer::text_width(sub, SECTION_SCALE);
    fb.draw_text(sub, (WIDTH as i32 - sw) / 2, 112, SECTION_SCALE);

    fb.draw_text("POST — MICROWAVE DATALINK", cx - 96, 140, SECTION_SCALE);
    fb.draw_text(
        "SENTRY-1..4 LINK CHECK .... OK",
        cx - 112,
        156,
        SECTION_SCALE,
    );

    let total = 40u32;
    let done = total.saturating_sub(state.boot_ticks_remaining);
    let bar_w = 200;
    let filled = (bar_w as u32 * done / total) as i32;
    let bx = cx - bar_w / 2;
    fb.rect_outline(bx, 175, bar_w, 14);
    if filled > 0 {
        fb.fill_rect(bx + 1, 176, filled.saturating_sub(1).max(0), 12, true);
    }

    fb.draw_text("PRESS ANY KEY", cx - 52, 210, SECTION_SCALE);
    let _ = HEIGHT; // keep height constant meaningful
}
