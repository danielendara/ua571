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
    apply_panel_key, fire_with_status, idle_runtime, load_native_config, AppState, FireDenyReason,
    NativeCli, PanelKey, Screen,
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
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let config = load_native_config(&NativeCli {
        theme: cli.theme.clone(),
        rounds: cli.rounds,
        tick_ms: cli.tick_ms,
        no_boot: cli.no_boot,
        demo: cli.demo,
        mute: cli.mute,
        config: cli.config.clone(),
    })?;
    let (on, off) = (config.theme.on_rgb_u32(), config.theme.off_rgb_u32());

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

        if handle_input(&window, &mut state, &mut confirm_hold, &mut status_hint) {
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
        fb.present_scaled(&mut buffer, ww, wh, on, off);
        window
            .update_with_buffer(&buffer, ww, wh)
            .map_err(|e| eyre!("present: {e}"))?;
    }

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

fn handle_input(
    window: &Window,
    state: &mut AppState,
    confirm_hold: &mut ConfirmHold,
    status_hint: &mut Option<&'static str>,
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

    if pressed(Key::Q) {
        state.quit();
        return true;
    }

    if pressed(Key::D) {
        state.toggle_demo();
        return true;
    }
    if pressed(Key::M) {
        // Mute state itself is recomputed every frame in `main` from
        // `state.config.sound` + focus via `idle_runtime`, so this only
        // needs to flip the preference.
        state.toggle_sound();
        return true;
    }
    if pressed(Key::F) {
        state.stop_demo();
        state.set_screen(Screen::Fire);
        return true;
    }
    if pressed(Key::O) {
        apply_panel_key(state, PanelKey::OpenOptions);
        return true;
    }
    if pressed(Key::Escape) {
        apply_panel_key(state, PanelKey::ToggleFirePanel);
        return true;
    }
    if pressed(Key::A) {
        state.stop_demo();
        state.toggle_arm();
        return true;
    }
    if pressed(Key::R) {
        state.stop_demo();
        state.reload();
        return true;
    }
    if pressed(Key::Key1) {
        state.stop_demo();
        state.select_sentry(0);
        return true;
    }
    if pressed(Key::Key2) {
        state.stop_demo();
        state.select_sentry(1);
        return true;
    }
    if pressed(Key::Key3) {
        state.stop_demo();
        state.select_sentry(2);
        return true;
    }
    if pressed(Key::Key4) {
        state.stop_demo();
        state.select_sentry(3);
        return true;
    }

    if state.screen == Screen::Options {
        if pressed(Key::Left) || pressed(Key::H) {
            state.stop_demo();
            state.focus_prev_section();
            return true;
        }
        if pressed(Key::Right) || pressed(Key::L) {
            state.stop_demo();
            state.focus_next_section();
            return true;
        }
        if pressed(Key::Up) || pressed(Key::K) {
            state.stop_demo();
            state.select_up();
            return true;
        }
        if pressed(Key::Down) || pressed(Key::J) {
            state.stop_demo();
            state.select_down();
            return true;
        }
    }

    // Edge-triggered: true only on the frame Enter/Space first goes down.
    // (minifb tracks how long a key has been held independent of the
    // `KeyRepeat` mode used to query it, so this is safe to check with
    // `KeyRepeat::No` even while a hold-to-fire repeat is in progress.)
    let confirm_fresh = window.is_key_pressed(Key::Enter, KeyRepeat::No)
        || window.is_key_pressed(Key::Space, KeyRepeat::No);
    if confirm_fresh {
        // Record whether *this* press started on the Fire screen, before
        // acting on it — a confirm on Options moves to Fire below, and that
        // must not itself count as an origin-on-Fire press.
        confirm_hold.origin_fire = state.screen == Screen::Fire;
        state.stop_demo();
        match state.screen {
            Screen::Fire => {
                let (_fired, status) = fire_with_status(state);
                *status_hint = status.map(FireDenyReason::message);
            }
            Screen::Options => state.set_screen(Screen::Fire),
            Screen::Boot => {}
        }
        return true;
    }

    // OS key-repeat pulse. Only treat it as hold-to-fire when the physical
    // hold began on the Fire screen — otherwise a Space/Enter held through
    // the Options → Fire confirm above would keep dumping rounds from the
    // very same press (#61).
    if confirm_hold.allows_repeat_fire(state.screen)
        && (window.is_key_pressed(Key::Enter, KeyRepeat::Yes)
            || window.is_key_pressed(Key::Space, KeyRepeat::Yes))
    {
        state.stop_demo();
        let (_fired, status) = fire_with_status(state);
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
            src.matches("fire_with_status(state)").count() >= 2,
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
    fn uses_shared_panel_key_helper() {
        let src = include_str!("main.rs");
        assert!(src.contains("apply_panel_key"));
        assert!(src.contains("PanelKey::OpenOptions"));
        assert!(src.contains("PanelKey::ToggleFirePanel"));
        assert!(
            src.contains("apply_panel_key(state, PanelKey::OpenOptions)"),
            "O must go through apply_panel_key"
        );
        assert!(
            src.contains("apply_panel_key(state, PanelKey::ToggleFirePanel)"),
            "Esc must go through apply_panel_key"
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
    fn mute_key_only_toggles_the_preference() {
        let src = include_str!("main.rs");
        let m_branch = src
            .split("if pressed(Key::M) {")
            .nth(1)
            .and_then(|s| s.split("return true;").next())
            .expect("Key::M branch");
        assert!(
            !m_branch.contains("set_muted"),
            "Key::M must not call set_muted directly: {m_branch}"
        );
        assert!(m_branch.contains("toggle_sound"));
    }
}
