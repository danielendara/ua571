//! Ignored helper: dump Options/Fire framebuffers for visual proof scripts.
//!
//! ```bash
//! UA571_DUMP_DIR=/tmp/ua571-dump cargo test -p ua571-render --test export_scene_dump -- --ignored --nocapture
//! ```

use std::path::PathBuf;
use ua571_core::{AppState, Config, Screen};
use ua571_render::{render, Framebuffer, HEIGHT, WIDTH};

fn dump_scene(out_dir: &PathBuf, tag: &str, screen: Screen) {
    let mut state = AppState::new(Config {
        show_boot: false,
        ..Config::default()
    });
    state.set_screen(screen);
    let mut fb = Framebuffer::new();
    render(&state, &mut fb);
    let path = out_dir.join(format!("{tag}.raw"));
    let mut bytes = Vec::with_capacity(8 + fb.pixels.len());
    bytes.extend_from_slice(&(WIDTH as u32).to_le_bytes());
    bytes.extend_from_slice(&(HEIGHT as u32).to_le_bytes());
    for p in &fb.pixels {
        bytes.push(if *p { 1 } else { 0 });
    }
    std::fs::write(&path, bytes).expect("write raw dump");
    eprintln!("wrote {}", path.display());
}

#[test]
#[ignore = "manual proof export; set UA571_DUMP_DIR"]
fn dump_options_and_fire() {
    let out_dir = std::env::var("UA571_DUMP_DIR")
        .map(PathBuf::from)
        .expect("UA571_DUMP_DIR");
    std::fs::create_dir_all(&out_dir).expect("mkdir");
    dump_scene(&out_dir, "options", Screen::Options);
    dump_scene(&out_dir, "fire", Screen::Fire);
}
