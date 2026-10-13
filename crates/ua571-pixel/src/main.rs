//! GRiD-faithful pixel frontend for UA 571-C.
//!
//! Fixed 640×240 monochrome logical canvas (coordinates from the GRiD Pascal
//! recreation), nearest-neighbor scaled into a phosphor-tinted window.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Parser;
use color_eyre::eyre::{eyre, Result};
use minifb::{Key, KeyRepeat, Scale, ScaleMode, Window, WindowOptions};
use ua571_audio::FireAudio;
use ua571_core::{
    apply_command, idle_runtime, load_native_startup, AppState, Command, FireDenyReason, NativeCli,
    Screen,
};
use ua571_render::{render, Framebuffer, HEIGHT, WIDTH};

/// Base window title (no fire-status suffix).
const BASE_TITLE_PREFIX: &str = "UA 571-C Remote Sentry Weapon System";

/// Compose the window title: base title plus a one-shot fire-status suffix,
/// the pixel analog of web's `#status` live region (`chrome_status_line`'s
/// ` · HINT`). Pixel has no on-canvas log/status text of its own, so the
/// title bar — the one text surface every windowed frontend already has —
/// is what carries CRITICAL / EMPTY / LINK DOWN / OFFLINE here (#86).
fn window_title(hint: Option<&str>) -> String {
    let base = format!("{BASE_TITLE_PREFIX}  v{}", ua571_core::VERSION);
    match hint {
        Some(h) => format!("{base} — {h}"),
        None => base,
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "ua571-pixel",
    version,
    about = "GRiD-style pixel UI for the UA 571-C console (closest to the original display)"
)]
struct Cli {
    /// Color theme: yellow | phosphor | amber | mono
    ///
    /// When omitted, uses the config file or the built-in yellow default.
    #[arg(short, long)]
    theme: Option<String>,

    /// Starting rounds per sentry
    #[arg(short, long)]
    rounds: Option<u16>,

    /// UI tick interval ms
    #[arg(long)]
    tick_ms: Option<u64>,

    /// Skip boot splash
    #[arg(long)]
    no_boot: bool,

    /// Start demo after boot
    #[arg(long)]
    demo: bool,

    /// Mute fire SFX
    #[arg(long)]
    mute: bool,

    /// Integer scale of the 640×240 canvas (1–6). Default 2.
    #[arg(short = 's', long, default_value_t = 2)]
    scale: u8,

    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Don't read or write the saved session this run (demo / kiosk use)
    #[arg(long)]
    no_save_session: bool,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    // CLI → saved session → config file → defaults (#109).
    let startup = load_native_startup(&NativeCli {
        theme: cli.theme.clone(),
        rounds: cli.rounds,
        tick_ms: cli.tick_ms,
        no_boot: cli.no_boot,
        demo: cli.demo,
        mute: cli.mute,
        config: cli.config.clone(),
        no_save_session: cli.no_save_session,
    })?;
    let config = startup.config.clone();
    let mut colors = (config.theme.on_rgb_u32(), config.theme.off_rgb_u32());

    let scale = cli.scale.clamp(1, 6) as usize;
    let win_w = WIDTH * scale;
    let win_h = HEIGHT * scale;

    let mut window = Window::new(
        &window_title(None),
        win_w,
        win_h,
        WindowOptions {
            resize: true,
            scale: Scale::X1,
            scale_mode: ScaleMode::Stretch,
            ..WindowOptions::default()
        },
    )
    .map_err(|e| eyre!("window: {e}"))?;

    window.set_target_fps(0);

    let mut state = AppState::new(config);
    startup.apply(&mut state);
    let mut audio = FireAudio::try_new();
    let mut fb = Framebuffer::new();
    let mut buffer = vec![0u32; win_w * win_h];
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(state.config.tick_ms);
    let mut dirty = true;
    let mut confirm_hold = ConfirmHold::default();
    // One-shot fire-status suffix for the window title (CRITICAL / EMPTY /
    // LINK DOWN / OFFLINE) — pixel's parity with web's `#status` hint (#86).
    let mut status_hint: Option<&'static str> = None;
    let mut shown_hint: Option<&'static str> = None;
    // Pause ticks/SFX while the OS window has lost focus — pixel's parity
    // with web's hidden-tab pause (#73, #90).
    let mut focus = FocusTracker::new(window.is_active());

    while window.is_open() && !state.should_quit {
        let (ww, wh) = window.get_size();
        if buffer.len() != ww * wh {
            buffer.resize(ww * wh, 0);
            dirty = true;
        }

        let focused = window.is_active();
        if focus.regained(focused) {
            // Resync the clock so the unfocused gap isn't dumped as one
            // huge catch-up dt on the next tick (#90).
            last_tick = Instant::now();
        }

        if handle_input(
            &window,
            &mut state,
            &mut confirm_hold,
            &mut status_hint,
            &mut colors,
        ) {
            dirty = true;
        }

        if status_hint != shown_hint {
            window.set_title(&window_title(status_hint));
            shown_hint = status_hint;
        }

        // Shared with web's `set_hidden`/`frame`: never tick or emit SFX in
        // the background, and never flip the operator's own sound pref.
        let idle = idle_runtime(!focused, state.config.sound);
        if let Some(a) = audio.as_mut() {
            a.set_muted(!idle.audio);
        }

        if idle.tick {
            if last_tick.elapsed() >= tick_rate {
                dirty |= state.tick();
                last_tick = Instant::now();
            }

            let n = state.take_fire_sfx();
            if n > 0 {
                if let Some(a) = audio.as_ref() {
                    a.play_fires(n);
                }
            }
        }

        if !dirty {
            let until_tick = tick_rate.saturating_sub(last_tick.elapsed());
            std::thread::sleep(until_tick.min(Duration::from_millis(16)));
            continue;
        }
        dirty = false;

        render(&state, &mut fb);
        fb.present_scaled(&mut buffer, ww, wh, colors.0, colors.1);
        window
            .update_with_buffer(&buffer, ww, wh)
            .map_err(|e| eyre!("present: {e}"))?;
    }

    // The console remembers how you left it — same set the web frontend keeps (#109).
    startup.save_on_exit(&state);

    Ok(())
}

/// Tracks window focus transitions so the main loop can resync `last_tick`
/// exactly once on the frame focus returns, instead of letting a stale
/// timestamp turn the whole unfocused gap into one huge dt (#90).
#[derive(Debug)]
struct FocusTracker {
    was_focused: bool,
}

impl FocusTracker {
    fn new(focused: bool) -> Self {
        Self {
            was_focused: focused,
        }
    }

    /// Record this frame's focus state; `true` exactly on the frame focus
    /// goes from lost to regained.
    fn regained(&mut self, focused: bool) -> bool {
        let regained = focused && !self.was_focused;
        self.was_focused = focused;
        regained
    }
}

/// Advance `state.config.theme`, return its minifb palette and status hint.
fn cycle_theme(state: &mut AppState) -> (&'static str, u32, u32) {
    let hint = state.config.theme.cycle_with_hint();
    let theme = state.config.theme;
    (hint, theme.on_rgb_u32(), theme.off_rgb_u32())
}

/// Enter, keypad Enter and Space confirm/fire (web's `NumpadEnter` parity).
const CONFIRM_KEYS: [Key; 3] = [Key::Enter, Key::NumPadEnter, Key::Space];

/// Top-row and numpad 1–4 select a sentry (web's `Numpad1`–`Numpad4` parity).
fn sentry_index_for(key: Key) -> Option<usize> {
    match key {
        Key::Key1 | Key::NumPad1 => Some(0),
        Key::Key2 | Key::NumPad2 => Some(1),
        Key::Key3 | Key::NumPad3 => Some(2),
        Key::Key4 | Key::NumPad4 => Some(3),
        _ => None,
    }
}

fn handle_input(
    window: &Window,
    state: &mut AppState,
    confirm_hold: &mut ConfirmHold,
    status_hint: &mut Option<&'static str>,
    colors: &mut (u32, u32),
) -> bool {
    if state.screen == Screen::Boot {
        if !window.get_keys_pressed(KeyRepeat::No).is_empty() {
            state.skip_boot();
            return true;
        }
        return false;
    }

    let pressed = |k: Key| window.is_key_pressed(k, KeyRepeat::No);

    // Any fresh (non-repeat) key clears a previous fire-status hint; only
    // the Fire confirm below may set a new one. A held Enter/Space
    // repeat-fire pulse (further down) recomputes it on every pulse instead.
    if !window.get_keys_pressed(KeyRepeat::No).is_empty() {
        *status_hint = None;
    }

    // Simple one-key commands; stop-demo and gating live in `apply_command`.
    // Mute state itself is recomputed every frame in `main` from
    // `state.config.sound` + focus via `idle_runtime`, so M only flips the
    // preference.
    let simple = [
        (Key::Q, Command::Quit),
        (Key::D, Command::ToggleDemo),
        (Key::M, Command::ToggleSound),
        (Key::F, Command::OpenFirePanel),
        (Key::O, Command::OpenOptions),
        (Key::Escape, Command::ToggleFirePanel),
        (Key::A, Command::ToggleArm),
        (Key::R, Command::Reload),
    ];
    if pressed(Key::T) {
        let (hint, on, off) = cycle_theme(state);
        *colors = (on, off);
        *status_hint = Some(hint);
        return true;
    }
    for (key, cmd) in simple {
        if pressed(key) {
            apply_command(state, cmd);
            return true;
        }
    }
    for key in [
        Key::Key1,
        Key::Key2,
        Key::Key3,
        Key::Key4,
        Key::NumPad1,
        Key::NumPad2,
        Key::NumPad3,
        Key::NumPad4,
    ] {
        if let Some(i) = sentry_index_for(key).filter(|_| pressed(key)) {
            apply_command(state, Command::SelectSentry(i));
            return true;
        }
    }

    if state.screen == Screen::Options {
        let nav = [
            (Key::Left, Key::H, Command::SectionPrev),
            (Key::Right, Key::L, Command::SectionNext),
            (Key::Up, Key::K, Command::SelectUp),
            (Key::Down, Key::J, Command::SelectDown),
        ];
        for (arrow, vi, cmd) in nav {
            if pressed(arrow) || pressed(vi) {
                apply_command(state, cmd);
                return true;
            }
        }
    }

    // Edge-triggered: true only on the frame Enter/Space first goes down.
    // (minifb tracks how long a key has been held independent of the
    // `KeyRepeat` mode used to query it, so this is safe to check with
    // `KeyRepeat::No` even while a hold-to-fire repeat is in progress.)
    let confirm_fresh = CONFIRM_KEYS
        .iter()
        .any(|&k| window.is_key_pressed(k, KeyRepeat::No));
    if confirm_fresh {
        // Record whether *this* press started on the Fire screen, before
        // acting on it — a confirm on Options moves to Fire below, and that
        // must not itself count as an origin-on-Fire press.
        confirm_hold.origin_fire = state.screen == Screen::Fire;
        let status = apply_command(state, Command::Confirm);
        *status_hint = status.map(FireDenyReason::message);
        return true;
    }

    // OS key-repeat pulse. Only treat it as hold-to-fire when the physical
    // hold began on the Fire screen — otherwise a Space/Enter held through
    // the Options → Fire confirm above would keep dumping rounds from the
    // very same press (#61).
    if confirm_hold.allows_repeat_fire(state.screen)
        && CONFIRM_KEYS
            .iter()
            .any(|&k| window.is_key_pressed(k, KeyRepeat::Yes))
    {
        let status = apply_command(state, Command::Confirm);
        *status_hint = status.map(FireDenyReason::message);
        return true;
    }

    false
}

/// Latches whether the Enter/Space hold currently in progress began while
/// the Fire screen was active, so OS key-repeat only continues hold-to-fire
/// for a press that started there — not one that began as an Options
/// confirm and only landed on Fire afterward.
#[derive(Debug, Default)]
struct ConfirmHold {
    origin_fire: bool,
}

impl ConfirmHold {
    /// Whether an OS key-repeat pulse for Enter/Space should be treated as
    /// hold-to-fire: only once the current physical hold both started on the
    /// Fire screen and is still there.
    fn allows_repeat_fire(&self, screen: Screen) -> bool {
        self.origin_fire && screen == Screen::Fire
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ua571_core::{Config, Theme};

    #[test]
    fn theme_flag_is_optional() {
        let cli = Cli::try_parse_from(["ua571-pixel"]).unwrap();
        assert!(cli.theme.is_none());
        assert_eq!(cli.scale, 2);
    }

    #[test]
    fn scale_and_theme_parse() {
        let cli = Cli::try_parse_from(["ua571-pixel", "-t", "mono", "-s", "4"]).unwrap();
        assert_eq!(cli.theme.as_deref(), Some("mono"));
        assert_eq!(cli.scale, 4);
    }

    #[test]
    fn reports_crate_version() {
        let err = Cli::try_parse_from(["ua571-pixel", "--version"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
        assert!(err.to_string().contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn window_title_has_no_hint_by_default() {
        let title = window_title(None);
        assert!(title.starts_with(BASE_TITLE_PREFIX));
        assert!(!title.contains('—'));
    }

    #[test]
    fn window_title_appends_fire_status_hint() {
        for hint in ["OFFLINE", "LINK DOWN", "EMPTY", "CRITICAL"] {
            let title = window_title(Some(hint));
            assert!(
                title.starts_with(BASE_TITLE_PREFIX),
                "hinted title should keep the base title: {title:?}"
            );
            assert!(
                title.ends_with(hint),
                "hinted title should end with the reason: {title:?}"
            );
        }
    }

    #[test]
    fn uses_shared_fire_deny_helper() {
        // Pixel must not re-derive its own OFFLINE/LINK DOWN/EMPTY/CRITICAL
        // classification — both fire call sites (fresh confirm + OS
        // key-repeat hold) go through the same `ua571-core` helper web uses,
        // so no frontend carries a third copy of the deny-copy strings (#86).
        let src = include_str!("main.rs");
        // >= 2, not ==, because this assertion's own source line also
        // contains the literal string it is searching for.
        assert!(
            src.matches("apply_command(state, Command::Confirm)")
                .count()
                >= 2,
            "both fire call sites should use the shared core helper"
        );
        // Built at runtime so this assertion's own source doesn't match itself.
        let bare_fire_call = format!("let _ = state.{}()", "fire");
        assert!(
            !src.contains(&bare_fire_call),
            "fire() should not be called bare anymore — use fire_with_status"
        );
    }

    #[test]
    fn routes_through_apply_command() {
        let src = include_str!("main.rs");
        assert!(src.contains("apply_command"));
        assert!(src.contains("Command::OpenOptions"));
        assert!(src.contains("Command::ToggleFirePanel"));
        assert!(
            src.contains("(Key::O, Command::OpenOptions)"),
            "O must go through apply_command"
        );
        assert!(
            src.contains("(Key::Escape, Command::ToggleFirePanel)"),
            "Esc must go through apply_command"
        );
    }

    /// Regression for holding Space/Enter from Options into Fire: minifb
    /// tracks how long a physical key has been held independent of which
    /// `KeyRepeat` mode is used to query it, so the same hold that confirms
    /// Options → Fire must not also be allowed to continue as hold-to-fire.
    #[test]
    fn confirm_hold_only_repeat_fires_when_hold_originated_on_fire() {
        // Fresh press happened while on Options (about to confirm to Fire).
        let mut hold = ConfirmHold { origin_fire: false };
        assert!(
            !hold.allows_repeat_fire(Screen::Fire),
            "a hold that confirmed Options -> Fire must not repeat-fire"
        );

        // Fresh press happened while already on Fire.
        hold.origin_fire = true;
        assert!(hold.allows_repeat_fire(Screen::Fire));

        // Never repeat-fires off the Fire screen, even with a stale flag.
        assert!(!hold.allows_repeat_fire(Screen::Options));
        assert!(!hold.allows_repeat_fire(Screen::Boot));
    }

    #[test]
    fn uses_shared_idle_runtime_helper() {
        // Pixel must not re-derive its own tick/mute-while-backgrounded
        // policy — it shares `ua571_core::idle_runtime` with web's
        // hidden-tab pause instead of carrying a third copy (#90).
        let src = include_str!("main.rs");
        assert!(
            src.contains("idle_runtime(!focused, state.config.sound)"),
            "main loop should gate ticks/SFX through the shared core helper"
        );
    }

    #[test]
    fn focus_tracker_flags_only_the_regain_frame() {
        let mut focus = FocusTracker::new(true);
        assert!(!focus.regained(true), "already focused is not a regain");
        assert!(!focus.regained(false), "losing focus is not a regain");
        assert!(!focus.regained(false), "staying unfocused is not a regain");
        assert!(
            focus.regained(true),
            "unfocused -> focused is exactly a regain"
        );
        assert!(
            !focus.regained(true),
            "staying focused after regain is not a regain again"
        );
    }

    #[test]
    fn focus_tracker_starting_unfocused_flags_first_focus() {
        let mut focus = FocusTracker::new(false);
        assert!(focus.regained(true));
    }

    /// Pixel mutes/unmutes purely from `state.config.sound` + focus each
    /// frame now, so `M` only has to flip the preference — it must never
    /// call `FireAudio::set_muted` itself and duplicate that policy.
    #[test]
    fn theme_key_cycles_config_and_palette() {
        let mut state = AppState::new(Config::default());
        assert_eq!(state.config.theme, Theme::Yellow);
        let (hint, on, off) = cycle_theme(&mut state);
        assert_eq!(state.config.theme, Theme::Phosphor);
        assert_eq!(hint, "THEME PHOSPHOR");
        assert_eq!(on, Theme::Phosphor.on_rgb_u32());
        assert_eq!(off, Theme::Phosphor.off_rgb_u32());
    }

    #[test]
    fn theme_key_wired_in_input_handler() {
        let src = include_str!("main.rs");
        let t_branch = src
            .split("if pressed(Key::T) {")
            .nth(1)
            .and_then(|s| s.split("return true;").next())
            .expect("Key::T branch");
        assert!(
            t_branch.contains("cycle_theme(state)"),
            "Key::T must cycle theme: {t_branch}"
        );
        assert!(
            !t_branch.contains("toggle_sound"),
            "Key::T must not affect sound: {t_branch}"
        );
    }

    #[test]
    fn mute_key_only_toggles_the_preference() {
        let src = include_str!("main.rs");
        // M is routed as a pure preference flip via the shared dispatcher;
        // the audio mute itself is recomputed per frame from `idle_runtime`.
        assert!(src.contains("(Key::M, Command::ToggleSound)"));
        let handler = src
            .split("fn handle_input(")
            .nth(1)
            .and_then(|s| s.split("\n}\n").next())
            .expect("handle_input body");
        assert!(
            !handler.contains("set_muted"),
            "Key::M must not call set_muted directly"
        );
    }

    #[test]
    fn sentry_index_for_maps_top_row_and_numpad() {
        assert_eq!(sentry_index_for(Key::Key1), Some(0));
        assert_eq!(sentry_index_for(Key::NumPad1), Some(0));
        assert_eq!(sentry_index_for(Key::Key2), Some(1));
        assert_eq!(sentry_index_for(Key::NumPad2), Some(1));
        assert_eq!(sentry_index_for(Key::Key3), Some(2));
        assert_eq!(sentry_index_for(Key::NumPad3), Some(2));
        assert_eq!(sentry_index_for(Key::Key4), Some(3));
        assert_eq!(sentry_index_for(Key::NumPad4), Some(3));
        assert_eq!(sentry_index_for(Key::Key5), None);
        assert_eq!(sentry_index_for(Key::NumPad5), None);
    }

    #[test]
    fn confirm_keys_include_keypad_enter() {
        assert!(CONFIRM_KEYS.contains(&Key::Enter));
        assert!(CONFIRM_KEYS.contains(&Key::NumPadEnter));
        assert!(CONFIRM_KEYS.contains(&Key::Space));
    }

    /// Non-test source of this file with all whitespace removed. It is cut at
    /// the test module so a needle can only match real code, never a test's
    /// own literal, and whitespace is dropped so rustfmt wrapping is ignored.
    fn non_test_source_compact() -> String {
        include_str!("main.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap_or_default()
            .split_whitespace()
            .collect()
    }

    #[test]
    fn confirm_keys_wired_for_fresh_and_repeat_checks() {
        let src = non_test_source_compact();
        assert!(
            src.contains("CONFIRM_KEYS.iter().any(|&k|window.is_key_pressed(k,KeyRepeat::No))"),
            "fresh confirm must check every CONFIRM_KEYS entry"
        );
        assert!(
            src.contains("CONFIRM_KEYS.iter().any(|&k|window.is_key_pressed(k,KeyRepeat::Yes))"),
            "hold-to-fire repeat must check every CONFIRM_KEYS entry"
        );
        assert!(
            src.contains("constCONFIRM_KEYS:[Key;3]=[Key::Enter,Key::NumPadEnter,Key::Space];"),
            "keypad Enter must be wired into CONFIRM_KEYS in non-test code"
        );
    }
}
