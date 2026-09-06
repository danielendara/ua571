//! Browser frontend: GRiD-faithful canvas rendered from Rust/WebAssembly.

#![forbid(unsafe_code)]

use ua571_core::sfx::{fire_burst_pcm, FIRE_CYCLIC_HZ};
use ua571_core::{AppState, Config, Screen, Theme};
use ua571_render::{render, Framebuffer, HEIGHT, WIDTH};
use wasm_bindgen::prelude::*;
use wasm_bindgen::Clamped;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AudioContext, AudioContextState, CanvasRenderingContext2d, HtmlCanvasElement, ImageData,
};
use web_time::{Duration, Instant};

/// Browser console app bound to a canvas element id.
#[wasm_bindgen]
pub struct Ua571Web {
    state: AppState,
    fb: Framebuffer,
    rgba: Vec<u8>,
    on_rgba: [u8; 4],
    off_rgba: [u8; 4],
    ctx: CanvasRenderingContext2d,
    last_tick: Instant,
    display_w: u32,
    display_h: u32,
    audio: Option<AudioContext>,
    fire_samples: Vec<f32>,
    fire_sample_rate: f32,
    confirm_gate: ConfirmRepeatGate,
    /// Brief chrome confirmation (e.g. Demo on/off) until the next key.
    status_hint: Option<&'static str>,
    /// Redraw the canvas on the next [`frame`] call.
    dirty: bool,
}

#[wasm_bindgen]
impl Ua571Web {
    /// Create and mount onto `#canvas_id`.
    ///
    /// * `theme` — `yellow` | `phosphor` | `amber` | `mono`
    /// * `scale` — integer pixel scale 1–6 (logical 640×240)
    /// * `demo` — start demo after boot
    /// * `skip_boot` — skip POST splash
    /// * `sound` — enable fire SFX (still muted by default in `Config`)
    #[wasm_bindgen(constructor)]
    pub fn new(
        canvas_id: &str,
        theme: &str,
        scale: u32,
        demo: bool,
        skip_boot: bool,
        sound: bool,
    ) -> Result<Ua571Web, JsValue> {
        console_error_panic_hook::set_once();

        let theme = Theme::parse(theme).unwrap_or(Theme::Yellow);
        let scale = scale.clamp(1, 6);
        let mut config = Config {
            theme,
            show_boot: !skip_boot,
            demo_on_start: demo,
            sound,
            ..Config::default()
        };
        config = config.validate();

        let document = web_sys::window()
            .ok_or_else(|| JsValue::from_str("no window"))?
            .document()
            .ok_or_else(|| JsValue::from_str("no document"))?;

        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str("canvas not found"))?
            .dyn_into::<HtmlCanvasElement>()?;

        let display_w = (WIDTH as u32) * scale;
        let display_h = (HEIGHT as u32) * scale;
        canvas.set_width(display_w);
        canvas.set_height(display_h);

        let ctx = canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("2d context missing"))?
            .dyn_into::<CanvasRenderingContext2d>()?;

        // Crisp nearest-neighbor upscaling when CSS sizes the canvas larger.
        ctx.set_image_smoothing_enabled(false);

        let (on_rgba, off_rgba) = (theme.on_rgba(), theme.off_rgba());
        let rgba = vec![0u8; (display_w * display_h * 4) as usize];

        // Decode PCM now; AudioContext is created on a user gesture (Sound / key).
        let (burst_sr, fire_samples) = fire_burst_pcm();

        Ok(Self {
            state: AppState::new(config),
            fb: Framebuffer::new(),
            rgba,
            on_rgba,
            off_rgba,
            ctx,
            last_tick: Instant::now(),
            display_w,
            display_h,
            audio: None,
            fire_samples,
            fire_sample_rate: burst_sr as f32,
            confirm_gate: ConfirmRepeatGate::default(),
            status_hint: None,
            dirty: true,
        })
    }

    /// Logical canvas width (before CSS).
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.display_w
    }

    /// Logical canvas height (before CSS).
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.display_h
    }

    /// Advance simulation (if tick elapsed) and redraw the canvas when needed.
    pub fn frame(&mut self) -> Result<(), JsValue> {
        let mut dirty = self.dirty;
        self.dirty = false;

        let tick = Duration::from_millis(self.state.config.tick_ms);
        if self.last_tick.elapsed() >= tick {
            dirty |= self.state.tick();
            self.last_tick = Instant::now();
        }

        let n = self.state.take_fire_sfx();
        if n > 0 {
            self.play_fires(n);
        }

        if !dirty {
            return Ok(());
        }

        render(&self.state, &mut self.fb);
        self.fb.present_rgba(
            &mut self.rgba,
            self.display_w as usize,
            self.display_h as usize,
            self.on_rgba,
            self.off_rgba,
        );

        // ImageData::new_with_u8_clamped_array_and_sh takes a Clamped slice.
        let image = ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(&self.rgba),
            self.display_w,
            self.display_h,
        )?;
        self.ctx.put_image_data(&image, 0.0, 0.0)?;
        Ok(())
    }

    /// Handle a browser keydown. `code` is `KeyboardEvent.code` (e.g. `KeyF`, `ArrowLeft`).
    ///
    /// `repeat` is `KeyboardEvent.repeat`. Space/Enter repeats hold-to-fire only
    /// when the originating (non-repeat) keydown was already on Fire.
    pub fn key_down(&mut self, code: &str, repeat: bool) {
        self.ensure_audio();
        self.resume_audio();
        if apply_key(
            &mut self.state,
            &mut self.confirm_gate,
            code,
            repeat,
            &mut self.status_hint,
        ) {
            self.dirty = true;
        }
    }

    /// Handle a browser keyup so a held Space/Enter can start a new hold-to-fire.
    pub fn key_up(&mut self, code: &str) {
        self.confirm_gate.key_up(code);
    }

    /// Whether the operator requested quit (`q`). Web page may ignore or show a message.
    #[wasm_bindgen(getter)]
    pub fn should_quit(&self) -> bool {
        self.state.should_quit
    }

    /// Active screen name for status UI: `boot` | `options` | `fire`.
    pub fn screen_name(&self) -> String {
        match self.state.screen {
            Screen::Boot => "boot".into(),
            Screen::Options => "options".into(),
            Screen::Fire => "fire".into(),
        }
    }

    /// Whether fire SFX is enabled (checkbox / `m` stay in sync).
    #[wasm_bindgen(getter)]
    pub fn sound_enabled(&self) -> bool {
        self.state.config.sound
    }

    /// Whether the Demo checkbox should be checked (stays in sync with `d`).
    ///
    /// During POST the player has not started yet, so this follows the
    /// scheduled `demo_on_start` flag instead of the live player.
    #[wasm_bindgen(getter)]
    pub fn demo_active(&self) -> bool {
        demo_checkbox_on(&self.state)
    }

    /// Start or stop demo auto-play without rebuilding the WASM app.
    ///
    /// During POST this only records the preference; auto-play still starts
    /// when the splash ends (or is skipped).
    pub fn set_demo(&mut self, on: bool) {
        apply_demo_checkbox(&mut self.state, on);
        self.dirty = true;
    }

    /// Enable or mute fire SFX. Enabling resumes the AudioContext after a gesture.
    pub fn set_sound(&mut self, on: bool) {
        if on != self.state.config.sound {
            self.state.toggle_sound();
        }
        if on {
            self.ensure_audio();
            self.resume_audio();
        }
        self.dirty = true;
    }

    /// Create (if needed) and await resume. Must run inside a user gesture.
    pub async fn unlock_audio(&mut self) -> Result<(), JsValue> {
        self.ensure_audio();
        let Some(ac) = self.audio.as_ref() else {
            return Ok(());
        };
        let p = ac.resume()?;
        JsFuture::from(p).await?;
        Ok(())
    }

    /// Short status line for HTML chrome.
    pub fn status_line(&self) -> String {
        chrome_status_line(&self.state, self.status_hint)
    }
}

impl Drop for Ua571Web {
    fn drop(&mut self) {
        if let Some(ac) = self.audio.take() {
            let _ = ac.close();
        }
    }
}

impl Ua571Web {
    fn ensure_audio(&mut self) {
        if self.audio.is_none() {
            self.audio = AudioContext::new().ok();
        }
    }

    fn resume_audio(&self) {
        if let Some(ac) = self.audio.as_ref() {
            let _ = ac.resume();
        }
    }

    fn play_fires(&mut self, count: u32) {
        if !self.state.config.sound || count == 0 {
            return;
        }
        let Some(ac) = self.audio.as_ref() else {
            return;
        };
        if ac.state() == AudioContextState::Suspended {
            let _ = ac.resume();
        }
        let n = count.min(6);
        let period = 1.0 / f64::from(FIRE_CYCLIC_HZ);
        let now = ac.current_time();
        for k in 0..n {
            let _ = play_buffer(
                ac,
                &self.fire_samples,
                self.fire_sample_rate,
                now + period * f64::from(k),
            );
        }
    }
}

fn play_buffer(
    ac: &AudioContext,
    samples: &[f32],
    sample_rate: f32,
    when: f64,
) -> Result<(), JsValue> {
    let n = samples.len() as u32;
    let buffer = ac.create_buffer(1, n, sample_rate)?;
    let channel = samples.to_vec();
    buffer.copy_to_channel(&channel, 0)?;

    let src = ac.create_buffer_source()?;
    src.set_buffer(Some(&buffer));
    src.connect_with_audio_node(&ac.destination())?;
    src.start_with_when(when)?;
    Ok(())
}

/// Workspace crate version for the web chrome (`v0.2.0`).
#[wasm_bindgen]
pub fn pkg_version() -> String {
    ua571_core::VERSION.to_string()
}

/// Chrome Demo checkbox: during POST reflect the scheduled auto-play flag.
fn demo_checkbox_on(state: &AppState) -> bool {
    if state.screen == Screen::Boot {
        state.config.demo_on_start
    } else {
        state.demo.is_active()
    }
}

/// HTML chrome status. During POST, DEMO/MANUAL follows the Demo checkbox
/// (scheduled `demo_on_start`) so the strip does not say MANUAL while the
/// box is still checked.
fn chrome_status_line(state: &AppState, hint: Option<&str>) -> String {
    let s = state.active_sentry();
    let audio = if state.config.sound { "SND" } else { "MUTE" };
    let mut line = format!(
        "S{} · {} rds · {} · {} · {} · {}",
        s.id,
        s.fire.rounds,
        s.options.system_mode.label(),
        if s.is_armed() { "ARMED" } else { "SAFE" },
        if demo_checkbox_on(state) {
            "DEMO"
        } else {
            "MANUAL"
        },
        audio
    );
    if let Some(hint) = hint {
        line.push_str(" · ");
        line.push_str(hint);
    }
    line
}

/// Apply the Demo checkbox. During POST only the pending flag is stored so
/// unchecking cancels auto-play that would otherwise start after splash.
fn apply_demo_checkbox(state: &mut AppState, on: bool) {
    state.config.demo_on_start = on;
    if state.screen == Screen::Boot {
        return;
    }
    if on != state.demo.is_active() {
        state.toggle_demo();
    }
}

fn is_confirm_code(code: &str) -> bool {
    matches!(code, "Enter" | "NumpadEnter" | "Space")
}

/// Space/Enter OS-repeat may fire only if the originating keydown was on Fire.
#[derive(Debug, Default)]
struct ConfirmRepeatGate {
    origin_fire: bool,
}

impl ConfirmRepeatGate {
    /// Whether this key event should be delivered to [`handle_key`].
    fn allow(&mut self, screen: Screen, code: &str, repeat: bool) -> bool {
        if repeat {
            return is_confirm_code(code) && self.origin_fire;
        }
        if is_confirm_code(code) {
            self.origin_fire = screen == Screen::Fire;
        }
        true
    }

    fn key_up(&mut self, code: &str) {
        if is_confirm_code(code) {
            self.origin_fire = false;
        }
    }
}

fn apply_key(
    state: &mut AppState,
    gate: &mut ConfirmRepeatGate,
    code: &str,
    repeat: bool,
    status_hint: &mut Option<&'static str>,
) -> bool {
    if !gate.allow(state.screen, code, repeat) {
        return false;
    }
    handle_key(state, code, status_hint);
    true
}

fn handle_key(state: &mut AppState, code: &str, status_hint: &mut Option<&'static str>) {
    *status_hint = None;
    if state.screen == Screen::Boot {
        state.skip_boot();
        return;
    }

    match code {
        "KeyQ" => {
            state.quit();
        }
        "KeyD" => {
            state.toggle_demo();
            *status_hint = Some(if demo_checkbox_on(state) {
                "Demo on"
            } else {
                "Demo off"
            });
        }
        "KeyM" => state.toggle_sound(),
        "KeyF" => {
            state.stop_demo();
            state.set_screen(Screen::Fire);
        }
        "KeyO" => {
            state.stop_demo();
            state.set_screen(Screen::Options);
        }
        "Escape" => {
            state.stop_demo();
            state.toggle_fire_panel();
        }
        "KeyA" => {
            state.stop_demo();
            state.toggle_arm();
        }
        "KeyR" => {
            state.stop_demo();
            state.reload();
        }
        "Digit1" | "Numpad1" => {
            state.stop_demo();
            state.select_sentry(0);
        }
        "Digit2" | "Numpad2" => {
            state.stop_demo();
            state.select_sentry(1);
        }
        "Digit3" | "Numpad3" => {
            state.stop_demo();
            state.select_sentry(2);
        }
        "Digit4" | "Numpad4" => {
            state.stop_demo();
            state.select_sentry(3);
        }
        "ArrowLeft" | "KeyH" if state.screen == Screen::Options => {
            state.stop_demo();
            state.focus_prev_section();
        }
        "ArrowRight" | "KeyL" if state.screen == Screen::Options => {
            state.stop_demo();
            state.focus_next_section();
        }
        "ArrowUp" | "KeyK" if state.screen == Screen::Options => {
            state.stop_demo();
            state.select_up();
        }
        "ArrowDown" | "KeyJ" if state.screen == Screen::Options => {
            state.stop_demo();
            state.select_down();
        }
        "Enter" | "NumpadEnter" | "Space" => {
            state.stop_demo();
            match state.screen {
                Screen::Fire => {
                    let _ = state.fire();
                }
                Screen::Options => state.set_screen(Screen::Fire),
                Screen::Boot => {}
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ua571_core::MenuSection;

    fn handle_key(state: &mut AppState, code: &str) {
        super::handle_key(state, code, &mut None);
    }

    fn apply_key(state: &mut AppState, gate: &mut ConfirmRepeatGate, code: &str, repeat: bool) {
        super::apply_key(state, gate, code, repeat, &mut None);
    }

    fn chrome_status_line(state: &AppState) -> String {
        super::chrome_status_line(state, None)
    }

    #[test]
    fn pkg_version_matches_core() {
        assert_eq!(pkg_version(), ua571_core::VERSION);
        assert_eq!(pkg_version(), "0.2.0");
    }

    #[test]
    fn synthesizes_non_empty_burst() {
        let s = ua571_core::sfx::synthesize_fire_burst(22_050, ua571_core::FIRE_SFX_MS, 0xC0FFEE);
        assert!(s.len() > 100);
        assert!(s.iter().any(|v| v.abs() > 0.05));
    }

    fn web_state() -> AppState {
        AppState::new(Config {
            show_boot: false,
            ..Config::default()
        })
    }

    #[test]
    fn keys_arm_fire_and_quit() {
        let mut state = web_state();
        handle_key(&mut state, "KeyA");
        assert!(state.active_sentry().is_armed());
        handle_key(&mut state, "KeyF");
        assert_eq!(state.screen, Screen::Fire);
        handle_key(&mut state, "Space");
        assert_eq!(state.fire_telemetry().rounds, 499);
        handle_key(&mut state, "KeyQ");
        assert!(state.should_quit);
    }

    #[test]
    fn keys_select_sentry_and_mute() {
        let mut state = web_state();
        handle_key(&mut state, "Digit3");
        assert_eq!(state.active_sentry().id, 3);
        assert!(!state.config.sound);
        handle_key(&mut state, "KeyM");
        assert!(state.config.sound);
        handle_key(&mut state, "KeyF");
        handle_key(&mut state, "Escape");
        assert_eq!(state.screen, Screen::Options);
    }

    #[test]
    fn options_arrows_and_hl_change_section_focus() {
        let mut state = web_state();
        assert_eq!(state.screen, Screen::Options);
        assert_eq!(state.active_sentry().options.focus, MenuSection::SystemMode);

        handle_key(&mut state, "ArrowRight");
        assert_eq!(
            state.active_sentry().options.focus,
            MenuSection::WeaponStatus
        );
        handle_key(&mut state, "ArrowLeft");
        assert_eq!(state.active_sentry().options.focus, MenuSection::SystemMode);

        handle_key(&mut state, "KeyL");
        assert_eq!(
            state.active_sentry().options.focus,
            MenuSection::WeaponStatus
        );
        handle_key(&mut state, "KeyH");
        assert_eq!(state.active_sentry().options.focus, MenuSection::SystemMode);

        handle_key(&mut state, "ArrowLeft");
        assert_eq!(
            state.active_sentry().options.focus,
            MenuSection::TargetSelect
        );
        handle_key(&mut state, "KeyL");
        assert_eq!(state.active_sentry().options.focus, MenuSection::SystemMode);
        assert_eq!(state.screen, Screen::Options);
        assert_eq!(state.active_sentry().id, 1);
    }

    #[test]
    fn digit_and_numpad_select_matching_sentry() {
        let mut state = web_state();
        assert_eq!(state.screen, Screen::Options);
        assert_eq!(state.active_sentry().id, 1);

        handle_key(&mut state, "Digit2");
        assert_eq!(state.active_sentry().id, 2);
        handle_key(&mut state, "Digit3");
        assert_eq!(state.active_sentry().id, 3);
        handle_key(&mut state, "Digit4");
        assert_eq!(state.active_sentry().id, 4);
        handle_key(&mut state, "Digit1");
        assert_eq!(state.active_sentry().id, 1);

        handle_key(&mut state, "Numpad2");
        assert_eq!(state.active_sentry().id, 2);
        handle_key(&mut state, "Numpad3");
        assert_eq!(state.active_sentry().id, 3);
        handle_key(&mut state, "Numpad4");
        assert_eq!(state.active_sentry().id, 4);
        handle_key(&mut state, "Numpad1");
        assert_eq!(state.active_sentry().id, 1);
        assert_eq!(state.screen, Screen::Options);
    }

    #[test]
    fn escape_on_options_returns_to_fire() {
        let mut state = web_state();
        assert_eq!(state.screen, Screen::Options);
        handle_key(&mut state, "Escape");
        assert_eq!(state.screen, Screen::Fire);
        handle_key(&mut state, "Escape");
        assert_eq!(state.screen, Screen::Options);
    }

    #[test]
    fn key_o_opens_options_from_fire_and_stays_on_options() {
        let mut state = web_state();
        handle_key(&mut state, "KeyO");
        assert_eq!(state.screen, Screen::Options);
        handle_key(&mut state, "KeyF");
        assert_eq!(state.screen, Screen::Fire);
        handle_key(&mut state, "KeyO");
        assert_eq!(state.screen, Screen::Options);
        handle_key(&mut state, "KeyO");
        assert_eq!(state.screen, Screen::Options);
    }

    #[test]
    fn keys_toggle_demo() {
        let mut state = web_state();
        handle_key(&mut state, "KeyD");
        assert!(state.demo.is_active());
        handle_key(&mut state, "KeyD");
        assert!(!state.demo.is_active());
    }

    #[test]
    fn demo_toggle_confirms_in_status_line() {
        let mut state = web_state();
        let mut hint = None;
        super::handle_key(&mut state, "KeyD", &mut hint);
        assert!(state.demo.is_active());
        let line = super::chrome_status_line(&state, hint);
        assert!(line.contains("Demo on"), "toggle on should confirm: {line}");
        assert!(
            line.contains("SAFE"),
            "Demo confirm must not drop SAFE chrome: {line}"
        );

        super::handle_key(&mut state, "KeyD", &mut hint);
        assert!(!state.demo.is_active());
        let line = super::chrome_status_line(&state, hint);
        assert!(
            line.contains("Demo off"),
            "toggle off should confirm: {line}"
        );

        super::handle_key(&mut state, "KeyM", &mut hint);
        let line = super::chrome_status_line(&state, hint);
        assert!(
            !line.contains("Demo on") && !line.contains("Demo off"),
            "hint should clear on the next action: {line}"
        );
    }

    #[test]
    fn demo_checkbox_stays_on_during_boot() {
        let state = AppState::new(Config {
            show_boot: true,
            demo_on_start: true,
            ..Config::default()
        });
        assert_eq!(state.screen, Screen::Boot);
        assert!(!state.demo.is_active());
        assert!(demo_checkbox_on(&state));
    }

    #[test]
    fn boot_status_line_says_demo_not_manual_when_checkbox_on() {
        let mut state = AppState::new(Config {
            show_boot: true,
            demo_on_start: true,
            ..Config::default()
        });
        assert!(!state.demo.is_active());
        let line = chrome_status_line(&state);
        assert!(
            line.contains("DEMO") && !line.contains("MANUAL"),
            "checked Demo must not read MANUAL during POST: {line}"
        );

        apply_demo_checkbox(&mut state, false);
        let line = chrome_status_line(&state);
        assert!(
            line.contains("MANUAL") && !line.contains("DEMO"),
            "unchecked Demo should read MANUAL during POST: {line}"
        );
    }

    #[test]
    fn unchecking_demo_during_boot_cancels_pending_autoplay() {
        let mut state = AppState::new(Config {
            show_boot: true,
            demo_on_start: true,
            ..Config::default()
        });
        apply_demo_checkbox(&mut state, false);
        assert!(!demo_checkbox_on(&state));
        state.skip_boot();
        assert!(!state.demo.is_active());
        assert!(!demo_checkbox_on(&state));
    }

    #[test]
    fn checking_demo_during_boot_starts_after_splash() {
        let mut state = AppState::new(Config {
            show_boot: true,
            demo_on_start: false,
            ..Config::default()
        });
        apply_demo_checkbox(&mut state, true);
        assert!(!state.demo.is_active());
        assert!(demo_checkbox_on(&state));
        state.skip_boot();
        assert!(state.demo.is_active());
        assert!(demo_checkbox_on(&state));
    }

    #[test]
    fn demo_checkbox_after_boot_toggles_player() {
        let mut state = web_state();
        apply_demo_checkbox(&mut state, true);
        assert!(state.demo.is_active());
        assert!(demo_checkbox_on(&state));
        apply_demo_checkbox(&mut state, false);
        assert!(!state.demo.is_active());
        assert!(!demo_checkbox_on(&state));
    }

    #[test]
    fn any_key_skips_boot() {
        let mut state = AppState::new(Config {
            show_boot: true,
            ..Config::default()
        });
        handle_key(&mut state, "KeyZ");
        assert_eq!(state.screen, Screen::Options);
    }

    #[test]
    fn space_during_boot_stays_on_options_with_demo() {
        let mut state = AppState::new(Config {
            show_boot: true,
            demo_on_start: true,
            ..Config::default()
        });
        handle_key(&mut state, "Space");
        assert_eq!(state.screen, Screen::Options);
        assert!(state.demo.is_active());
    }

    #[test]
    fn space_on_options_opens_fire_and_stops_demo() {
        let mut state = web_state();
        handle_key(&mut state, "KeyD");
        assert_eq!(state.screen, Screen::Options);
        assert!(state.demo.is_active());
        handle_key(&mut state, "Space");
        assert_eq!(state.screen, Screen::Fire);
        assert!(!state.demo.is_active());
    }

    #[test]
    fn enter_on_fire_expends_a_round() {
        let mut state = web_state();
        handle_key(&mut state, "KeyA");
        handle_key(&mut state, "KeyF");
        handle_key(&mut state, "Enter");
        assert_eq!(state.fire_telemetry().rounds, 499);
        handle_key(&mut state, "NumpadEnter");
        assert_eq!(state.fire_telemetry().rounds, 498);
    }

    #[test]
    fn space_repeat_through_post_stays_on_options_with_demo() {
        let mut state = AppState::new(Config {
            show_boot: true,
            demo_on_start: true,
            ..Config::default()
        });
        let mut gate = ConfirmRepeatGate::default();
        apply_key(&mut state, &mut gate, "Space", false);
        assert_eq!(state.screen, Screen::Options);
        assert!(state.demo.is_active());
        apply_key(&mut state, &mut gate, "Space", true);
        apply_key(&mut state, &mut gate, "Enter", true);
        assert_eq!(state.screen, Screen::Options);
        assert!(state.demo.is_active());
    }

    #[test]
    fn space_repeat_after_options_confirm_does_not_fire() {
        let mut state = web_state();
        let mut gate = ConfirmRepeatGate::default();
        apply_key(&mut state, &mut gate, "KeyA", false);
        apply_key(&mut state, &mut gate, "Space", false);
        assert_eq!(state.screen, Screen::Fire);
        let rounds = state.fire_telemetry().rounds;
        apply_key(&mut state, &mut gate, "Space", true);
        apply_key(&mut state, &mut gate, "Enter", true);
        assert_eq!(state.fire_telemetry().rounds, rounds);
    }

    #[test]
    fn space_repeat_after_options_confirm_unarmed_does_not_log_safe() {
        let mut state = web_state();
        let mut gate = ConfirmRepeatGate::default();
        apply_key(&mut state, &mut gate, "Space", false);
        assert_eq!(state.screen, Screen::Fire);
        apply_key(&mut state, &mut gate, "Space", true);
        assert!(
            !state
                .log
                .recent(16)
                .iter()
                .any(|e| e.kind.to_string().contains("CANNOT FIRE")),
            "Options hold must not spam SAFE fire attempts"
        );
    }

    #[test]
    fn space_repeat_on_fire_hold_to_fires_until_keyup() {
        let mut state = web_state();
        let mut gate = ConfirmRepeatGate::default();
        apply_key(&mut state, &mut gate, "KeyA", false);
        apply_key(&mut state, &mut gate, "KeyF", false);
        apply_key(&mut state, &mut gate, "Space", false);
        assert_eq!(state.fire_telemetry().rounds, 499);
        apply_key(&mut state, &mut gate, "Space", true);
        assert_eq!(state.fire_telemetry().rounds, 498);
        gate.key_up("Space");
        apply_key(&mut state, &mut gate, "Space", true);
        assert_eq!(state.fire_telemetry().rounds, 498);
        apply_key(&mut state, &mut gate, "Space", false);
        assert_eq!(state.fire_telemetry().rounds, 497);
    }

    #[test]
    fn arrow_repeat_is_ignored() {
        let mut state = web_state();
        let mut gate = ConfirmRepeatGate::default();
        let before = state.active_sentry().options;
        apply_key(&mut state, &mut gate, "ArrowDown", true);
        assert_eq!(state.active_sentry().options, before);
        apply_key(&mut state, &mut gate, "ArrowDown", false);
        assert_ne!(state.active_sentry().options, before);
    }

    #[test]
    fn status_element_is_polite_named_live_region() {
        let html = include_str!("../../../web/index.html");
        let status = html
            .split("<p")
            .find(|chunk| chunk.contains(r#"id="status""#))
            .unwrap_or("");
        assert!(
            status.contains(r#"role="status""#),
            "status node must be a live region"
        );
        assert!(
            status.contains(r#"aria-live="polite""#),
            "status live region must be polite"
        );
        assert!(
            status.contains(r#"aria-label="Game status""#),
            "status region must have an accessible name"
        );
    }

    /// `web/main.test.js` — live-region writes and Demo on/off after `d`.
    #[cfg(target_os = "linux")]
    #[test]
    fn status_live_region_node_tests() {
        use std::path::PathBuf;
        use std::process::Command;

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let spec = root.join("web/main.test.js");
        let output = Command::new("node")
            .arg("--test")
            .arg(&spec)
            .current_dir(&root)
            .output()
            .expect("spawn node (install Node.js to run web chrome tests)");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "node --test web/main.test.js failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }
}
