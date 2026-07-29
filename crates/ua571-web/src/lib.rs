//! Browser frontend: GRiD-faithful canvas rendered from Rust/WebAssembly.

#![forbid(unsafe_code)]

use ua571_core::{AppState, Config, Screen, Theme, WeaponStatus};
use ua571_render::{render, Framebuffer, HEIGHT, WIDTH};
use wasm_bindgen::prelude::*;
use wasm_bindgen::Clamped;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};
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

        let theme = Theme::parse(theme).unwrap_or(Theme::Phosphor);
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

        let (on_rgba, off_rgba) = theme_rgba(theme);
        let rgba = vec![0u8; (display_w * display_h * 4) as usize];

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
        format!(
            "S{} · {} rds · {} · {} · {}",
            s.id,
            s.fire.rounds,
            s.options.system_mode.label(),
            if s.is_armed() { "ARMED" } else { "SAFE" },
            if self.state.demo.is_active() {
                "DEMO"
            } else {
                "MANUAL"
            }
        )
    }
}

fn theme_rgba(theme: Theme) -> ([u8; 4], [u8; 4]) {
    let on = match theme {
        Theme::Phosphor => [0x50, 0xfa, 0x7b, 0xff],
        Theme::Amber => [0xff, 0xb0, 0x00, 0xff],
        Theme::Mono => [0xe0, 0xe0, 0xe0, 0xff],
    };
    let off = [0x00, 0x00, 0x00, 0xff];
    (on, off)
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
            toggle_arm(state);
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

fn toggle_arm(state: &mut AppState) {
    if let Some(s) = state.active_sentry_mut() {
        s.options.weapon_status = match s.options.weapon_status {
            WeaponStatus::Safe => WeaponStatus::Armed,
            WeaponStatus::Armed => WeaponStatus::Safe,
        };
        let id = s.id;
        match s.options.weapon_status {
            WeaponStatus::Armed => state.log.push(ua571_core::LogKind::Armed { sentry: id }),
            WeaponStatus::Safe => state.log.push(ua571_core::LogKind::Safe { sentry: id }),
        }
    }
}
