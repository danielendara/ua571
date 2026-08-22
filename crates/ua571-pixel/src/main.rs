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
use ua571_core::{load_native_config, AppState, NativeCli, Screen};
use ua571_render::{render, Framebuffer, HEIGHT, WIDTH};

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
        &format!(
            "UA 571-C Remote Sentry Weapon System  v{}",
            ua571_core::VERSION
        ),
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

    window.set_target_fps(60);

    let mut state = AppState::new(config);
    let mut audio = FireAudio::try_new();
    if let Some(a) = audio.as_mut() {
        a.set_muted(!state.config.sound);
    }
    let mut fb = Framebuffer::new();
    let mut buffer = vec![0u32; win_w * win_h];
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(state.config.tick_ms);

    while window.is_open() && !state.should_quit {
        let (ww, wh) = window.get_size();
        if buffer.len() != ww * wh {
            buffer.resize(ww * wh, 0);
        }

        handle_input(&window, &mut state, audio.as_mut());

        if last_tick.elapsed() >= tick_rate {
            state.tick();
            last_tick = Instant::now();
        }

        let n = state.take_fire_sfx();
        if n > 0 {
            if let Some(a) = audio.as_ref() {
                a.play_fires(n);
            }
        }

        render(&state, &mut fb);
        fb.present_scaled(&mut buffer, ww, wh, on, off);
        window
            .update_with_buffer(&buffer, ww, wh)
            .map_err(|e| eyre!("present: {e}"))?;
    }

    Ok(())
}

fn handle_input(window: &Window, state: &mut AppState, audio: Option<&mut FireAudio>) {
    if state.screen == Screen::Boot {
        if !window.get_keys_pressed(KeyRepeat::No).is_empty() {
            state.skip_boot();
        }
        return;
    }

    let pressed = |k: Key| window.is_key_pressed(k, KeyRepeat::No);

    if pressed(Key::Q) {
        state.quit();
        return;
    }

    if pressed(Key::D) {
        state.toggle_demo();
    }
    if pressed(Key::M) {
        state.toggle_sound();
        if let Some(a) = audio {
            a.set_muted(!state.config.sound);
        }
    }
    if pressed(Key::F) {
        state.stop_demo();
        state.set_screen(Screen::Fire);
    }
    if pressed(Key::O) || pressed(Key::Escape) {
        state.stop_demo();
        state.set_screen(Screen::Options);
    }
    if pressed(Key::A) {
        state.stop_demo();
        state.toggle_arm();
    }
    if pressed(Key::R) {
        state.stop_demo();
        state.reload();
    }
    if pressed(Key::Key1) {
        state.stop_demo();
        state.select_sentry(0);
    }
    if pressed(Key::Key2) {
        state.stop_demo();
        state.select_sentry(1);
    }
    if pressed(Key::Key3) {
        state.stop_demo();
        state.select_sentry(2);
    }
    if pressed(Key::Key4) {
        state.stop_demo();
        state.select_sentry(3);
    }

    if state.screen == Screen::Options {
        if pressed(Key::Left) || pressed(Key::H) {
            state.stop_demo();
            state.focus_prev_section();
        }
        if pressed(Key::Right) || pressed(Key::L) {
            state.stop_demo();
            state.focus_next_section();
        }
        if pressed(Key::Up) || pressed(Key::K) {
            state.stop_demo();
            state.select_up();
        }
        if pressed(Key::Down) || pressed(Key::J) {
            state.stop_demo();
            state.select_down();
        }
    }

    if pressed(Key::Enter) || pressed(Key::Space) {
        state.stop_demo();
        match state.screen {
            Screen::Fire => {
                let _ = state.fire();
            }
            Screen::Options => state.set_screen(Screen::Fire),
            Screen::Boot => {}
        }
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
}
