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

    /// Advance simulation (if tick elapsed) and redraw the canvas.
    pub fn frame(&mut self) -> Result<(), JsValue> {
        let tick = Duration::from_millis(self.state.config.tick_ms);
        if self.last_tick.elapsed() >= tick {
            self.state.tick();
            self.last_tick = Instant::now();
        }

        let n = self.state.take_fire_sfx();
        if n > 0 {
            self.play_fires(n);
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
    pub fn key_down(&mut self, code: &str) {
        self.ensure_audio();
        self.resume_audio();
        handle_key(&mut self.state, code);
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
        chrome_status_line(&self.state)
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
fn chrome_status_line(state: &AppState) -> String {
    let s = state.active_sentry();
    let audio = if state.config.sound { "SND" } else { "MUTE" };
    format!(
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
    )
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

fn handle_key(state: &mut AppState, code: &str) {
    if state.screen == Screen::Boot {
        state.skip_boot();
        return;
    }

    match code {
        "KeyQ" => {
            state.quit();
        }
        "KeyD" => state.toggle_demo(),
        "KeyM" => state.toggle_sound(),
        "KeyF" => {
            state.stop_demo();
            state.set_screen(Screen::Fire);
        }
        "KeyO" | "Escape" => {
            state.stop_demo();
            state.set_screen(Screen::Options);
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
        handle_key(&mut state, "Escape");
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
}
