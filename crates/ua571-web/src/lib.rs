//! Browser frontend: GRiD-faithful canvas rendered from Rust/WebAssembly.

#![forbid(unsafe_code)]

use ua571_core::sfx::{synthesize_fire_burst, FIRE_CYCLIC_HZ, FIRE_SFX_MS};
use ua571_core::{AppState, Config, Screen, Theme};
use ua571_render::{render, Framebuffer, HEIGHT, WIDTH};
use wasm_bindgen::prelude::*;
use wasm_bindgen::Clamped;
use web_sys::{AudioContext, CanvasRenderingContext2d, HtmlCanvasElement, ImageData};
use web_time::{Duration, Instant};

const VARIANT_COUNT: usize = 6;

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
    fire_variants: Vec<Vec<f32>>,
    next_variant: usize,
    sample_rate: f32,
}

#[wasm_bindgen]
impl Ua571Web {
    /// Create and mount onto `#canvas_id`.
    ///
    /// * `theme` — `phosphor` | `amber` | `mono`
    /// * `scale` — integer pixel scale 1–6 (logical 640×240)
    /// * `demo` — start demo after boot
    /// * `skip_boot` — skip POST splash
    #[wasm_bindgen(constructor)]
    pub fn new(
        canvas_id: &str,
        theme: &str,
        scale: u32,
        demo: bool,
        skip_boot: bool,
    ) -> Result<Ua571Web, JsValue> {
        console_error_panic_hook::set_once();

        let theme = Theme::parse(theme).unwrap_or(Theme::Yellow);
        let scale = scale.clamp(1, 6);
        let mut config = Config {
            theme,
            show_boot: !skip_boot,
            demo_on_start: demo,
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

        // Audio is optional: some environments block it until a user gesture.
        let (audio, sample_rate) = match AudioContext::new() {
            Ok(ac) => {
                let sr = ac.sample_rate();
                (Some(ac), sr)
            }
            Err(_) => (None, 22_050.0),
        };
        let fire_variants = (0..VARIANT_COUNT)
            .map(|i| {
                synthesize_fire_burst(
                    sample_rate as u32,
                    FIRE_SFX_MS,
                    0xA57E_u32.wrapping_mul(i as u32 + 1).wrapping_add(0xC0FFEE),
                )
            })
            .collect();

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
            audio,
            fire_variants,
            next_variant: 0,
            sample_rate,
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
            self.play_fires(n); // &mut self — rotates pulse variants
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
        // Browsers suspend AudioContext until a user gesture — resume on any key.
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

    /// Short status line for HTML chrome.
    pub fn status_line(&self) -> String {
        let s = self.state.active_sentry();
        let audio = if self.state.config.sound {
            "SND"
        } else {
            "MUTE"
        };
        format!(
            "S{} · {} rds · {} · {} · {} · {}",
            s.id,
            s.fire.rounds,
            s.options.system_mode.label(),
            if s.is_armed() { "ARMED" } else { "SAFE" },
            if self.state.demo.is_active() {
                "DEMO"
            } else {
                "MANUAL"
            },
            audio
        )
    }
}

impl Ua571Web {
    fn resume_audio(&self) {
        if let Some(ac) = self.audio.as_ref() {
            let _ = ac.resume();
        }
    }

    fn play_fires(&mut self, count: u32) {
        if !self.state.config.sound {
            return;
        }
        let Some(ac) = self.audio.as_ref() else {
            return;
        };
        let n = count.min(6) as usize;
        if n == 0 {
            return;
        }
        let period = 1.0 / f64::from(FIRE_CYCLIC_HZ);
        let now = ac.current_time();
        for k in 0..n {
            let i = self.next_variant;
            self.next_variant = (self.next_variant + 1) % self.fire_variants.len();
            let samples = &self.fire_variants[i];
            let when = now + period * k as f64;
            let _ = play_buffer(ac, samples, self.sample_rate, when);
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
    fn synthesizes_non_empty_burst() {
        let s = synthesize_fire_burst(22_050, FIRE_SFX_MS, 0xC0FFEE);
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
        handle_key(&mut state, "KeyM");
        assert!(!state.config.sound);
        handle_key(&mut state, "Escape");
        assert_eq!(state.screen, Screen::Options);
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
}
